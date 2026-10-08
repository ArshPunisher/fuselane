//! Encryption for a share (FUSE-SEND.md §4). The torrent carries:
//!
//! ```text
//! [header block: u32 LE length, nonce, sealed header, zero padding to 1024 bytes]
//! [content XORed with a keystream]
//! ```
//!
//! - The sealed header (name, size, BLAKE3 of the plaintext) uses XChaCha20-Poly1305
//!   with a random nonce, so a wrong key or a changed byte is caught at once.
//! - The content uses the XChaCha20 keystream at each byte's offset, so any torrent
//!   piece can be encrypted or decrypted on its own, without a second copy on disk.
//!   Torrent piece hashes cover the ciphertext; the BLAKE3 hash checks the result.
//!
//! The two keys are derived from the link's key with BLAKE3, so one key is never
//! used for two purposes, and a fixed content nonce is safe because every share has
//! its own random key.

use chacha20::XChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

const CONTENT_CONTEXT: &str = "Fuselane send v1 2026-10-09 content";
const HEADER_CONTEXT: &str = "Fuselane send v1 2026-10-09 header";
const HEADER_VERSION: u8 = 1;
const NONCE: usize = 24;
/// Longest file name accepted (bytes), as most file systems allow.
pub const MAX_NAME: usize = 255;
/// The header block's fixed size, so a receiver knows where content starts (and
/// the content's size, from the torrent's length) before it can decrypt anything.
pub const HEADER_BLOCK: usize = 1024;

/// What the receiver learns only after decrypting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub size: u64,
    /// BLAKE3 of the plaintext, checked once every byte has arrived.
    pub hash: [u8; 32],
}

/// Why a share couldn't be opened, worded for the receiver (ERRORS.md).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptError {
    #[error(
        "This file didn't check out: the link's key doesn't match, or the data was changed. Ask the sender for a new link."
    )]
    Tampered,
    #[error(
        "The file's name isn't safe to save (\"{0}\"). Ask the sender to rename it and share it again."
    )]
    BadName(String),
    #[error("This share was made by a newer Fuselane. Update Fuselane to open it.")]
    TooNew,
}

/// The two keys a share uses, derived from the link's key.
pub struct Keys {
    content: [u8; 32],
    header: [u8; 32],
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keys(<hidden>)")
    }
}

impl Keys {
    pub fn derive(link_key: &[u8; 32]) -> Keys {
        Keys {
            content: blake3::derive_key(CONTENT_CONTEXT, link_key),
            header: blake3::derive_key(HEADER_CONTEXT, link_key),
        }
    }

    /// XORs the content keystream into `buf`, which sits at `offset` in the
    /// content (after the header). Encrypting and decrypting are the same call.
    pub fn apply(&self, offset: u64, buf: &mut [u8]) {
        let mut c = XChaCha20::new(&self.content.into(), &[0u8; NONCE].into());
        c.seek(offset);
        c.apply_keystream(buf);
    }

    /// The header block: length prefix, then nonce and sealed header.
    pub fn seal(&self, h: &Header) -> Result<Vec<u8>, CryptError> {
        check_name(&h.name)?;
        let mut plain = Vec::with_capacity(1 + 8 + 32 + 2 + h.name.len());
        plain.push(HEADER_VERSION);
        plain.extend_from_slice(&h.size.to_le_bytes());
        plain.extend_from_slice(&h.hash);
        plain.extend_from_slice(&(h.name.len() as u16).to_le_bytes());
        plain.extend_from_slice(h.name.as_bytes());
        let mut nonce = [0u8; NONCE];
        getrandom::fill(&mut nonce).map_err(|_| CryptError::Tampered)?;
        let sealed = XChaCha20Poly1305::new(&self.header.into())
            .encrypt(&XNonce::from(nonce), plain.as_slice())
            .map_err(|_| CryptError::Tampered)?;
        let len = (NONCE + sealed.len()) as u32;
        let mut out = Vec::with_capacity(HEADER_BLOCK);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        // A 255-byte name seals to about 350 bytes, far below the block.
        out.resize(HEADER_BLOCK, 0);
        Ok(out)
    }

    /// Reads the header block (the first `HEADER_BLOCK` bytes of the torrent's data).
    pub fn open(&self, data: &[u8]) -> Result<Header, CryptError> {
        let len = data
            .get(..4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .ok_or(CryptError::Tampered)?;
        if !(NONCE + 16..=HEADER_BLOCK - 4).contains(&len) {
            return Err(CryptError::Tampered);
        }
        let block = data.get(4..4 + len).ok_or(CryptError::Tampered)?;
        let (nonce, sealed) = block.split_at(NONCE);
        let mut n = [0u8; NONCE];
        n.copy_from_slice(nonce);
        let plain = XChaCha20Poly1305::new(&self.header.into())
            .decrypt(&XNonce::from(n), sealed)
            .map_err(|_| CryptError::Tampered)?;
        // Authenticated from here on: only our own writer could have made it.
        let (&version, rest) = plain.split_first().ok_or(CryptError::Tampered)?;
        if version != HEADER_VERSION {
            return Err(CryptError::TooNew);
        }
        if rest.len() < 8 + 32 + 2 {
            return Err(CryptError::Tampered);
        }
        let size = u64::from_le_bytes(rest[..8].try_into().map_err(|_| CryptError::Tampered)?);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&rest[8..40]);
        let name_len = u16::from_le_bytes([rest[40], rest[41]]) as usize;
        let name = rest.get(42..42 + name_len).ok_or(CryptError::Tampered)?;
        let name = String::from_utf8(name.to_vec()).map_err(|_| CryptError::Tampered)?;
        // Checked again on this side: a sender's app could be modified.
        check_name(&name)?;
        Ok(Header { name, size, hash })
    }
}

/// A plain file name, never a path: nothing a receiver could be tricked into
/// writing outside the folder they chose.
pub fn check_name(name: &str) -> Result<(), CryptError> {
    let bad = name.is_empty()
        || name.len() > MAX_NAME
        || name == "."
        || name == ".."
        || name.trim() != name
        || name.ends_with('.')
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        || is_windows_device(name);
    if bad {
        Err(CryptError::BadName(name.chars().take(80).collect()))
    } else {
        Ok(())
    }
}

/// Names Windows treats as devices, with or without an extension (CON, com1.txt).
fn is_windows_device(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

/// BLAKE3 of the plaintext, fed in order as pieces are decrypted.
pub fn hasher() -> blake3::Hasher {
    blake3::Hasher::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK_KEY: [u8; 32] = [42; 32];
    const PINNED: [u8; 16] = [
        0x29, 0x01, 0xa3, 0x70, 0x58, 0x59, 0x69, 0x18, 0x72, 0xf2, 0xb1, 0x39, 0xaf, 0xbf, 0x7c,
        0x45,
    ];

    fn header() -> Header {
        Header {
            name: "holiday video.mp4".into(),
            size: 123_456,
            hash: *blake3::hash(b"plaintext").as_bytes(),
        }
    }

    #[test]
    fn a_sealed_header_opens_with_the_same_key() {
        let k = Keys::derive(&LINK_KEY);
        let block = k.seal(&header()).unwrap();
        assert_eq!(block.len(), HEADER_BLOCK, "always the same size");
        let mut data = block.clone();
        data.extend_from_slice(b"content follows");
        assert_eq!(k.open(&data), Ok(header()));
        let long = Header {
            name: "x".repeat(MAX_NAME),
            ..header()
        };
        assert_eq!(
            k.open(&k.seal(&long).unwrap()),
            Ok(long),
            "longest name fits"
        );
    }

    #[test]
    fn the_wrong_key_or_any_changed_byte_is_caught() {
        let k = Keys::derive(&LINK_KEY);
        let block = k.seal(&header()).unwrap();
        assert_eq!(
            Keys::derive(&[43; 32]).open(&block),
            Err(CryptError::Tampered)
        );
        let used = 4 + u32::from_le_bytes(block[..4].try_into().unwrap()) as usize;
        for i in 4..used {
            let mut b = block.clone();
            b[i] ^= 0x01;
            assert_eq!(k.open(&b), Err(CryptError::Tampered), "byte {i}");
        }
    }

    #[test]
    fn short_or_oversized_header_blocks_are_refused_without_panicking() {
        let k = Keys::derive(&LINK_KEY);
        let block = k.seal(&header()).unwrap();
        let used = 4 + u32::from_le_bytes(block[..4].try_into().unwrap()) as usize;
        for cut in [0, 3, 4, 10, used - 1] {
            assert_eq!(
                k.open(&block[..cut]),
                Err(CryptError::Tampered),
                "cut at {cut}"
            );
        }
        let mut huge = block.clone();
        huge[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(k.open(&huge), Err(CryptError::Tampered));
        let mut tiny = block;
        tiny[..4].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(k.open(&tiny), Err(CryptError::Tampered));
    }

    #[test]
    fn two_seals_of_one_header_differ_but_both_open() {
        let k = Keys::derive(&LINK_KEY);
        let (a, b) = (k.seal(&header()).unwrap(), k.seal(&header()).unwrap());
        assert_ne!(a, b, "fresh nonce each time");
        assert_eq!(k.open(&a), k.open(&b));
    }

    /// The core property: encrypting piece by piece at any offset gives exactly the
    /// bytes of encrypting the whole stream, and applying again restores it.
    #[test]
    fn pieces_at_any_offset_match_the_whole_stream() {
        let k = Keys::derive(&LINK_KEY);
        let plain: Vec<u8> = (0..10_000u32).map(|i| (i * 7 + 3) as u8).collect();
        let mut whole = plain.clone();
        k.apply(0, &mut whole);
        assert_ne!(whole, plain);
        // Block edges (64 bytes), odd sizes and a big jump.
        for (start, len) in [
            (0, 1),
            (63, 2),
            (64, 64),
            (65, 1000),
            (127, 129),
            (4095, 3),
            (9_999, 1),
            (1, 9_999),
        ] {
            let mut piece = plain[start..start + len].to_vec();
            k.apply(start as u64, &mut piece);
            assert_eq!(piece, whole[start..start + len], "piece {start}+{len}");
            k.apply(start as u64, &mut piece);
            assert_eq!(piece, plain[start..start + len], "decrypt {start}+{len}");
        }
    }

    #[test]
    fn offsets_past_4_gib_work() {
        let k = Keys::derive(&LINK_KEY);
        let far = 5 * 1024 * 1024 * 1024u64 + 17;
        let mut a = vec![0u8; 100];
        k.apply(far, &mut a);
        let mut b = vec![0u8; 50];
        k.apply(far + 50, &mut b);
        assert_eq!(a[50..], b[..]);
        assert!(a.iter().any(|&x| x != 0));
    }

    #[test]
    fn different_links_give_different_keystreams_and_keys_stay_hidden() {
        let (a, b) = (Keys::derive(&[1; 32]), Keys::derive(&[2; 32]));
        let (mut x, mut y) = (vec![0u8; 64], vec![0u8; 64]);
        a.apply(0, &mut x);
        b.apply(0, &mut y);
        assert_ne!(x, y);
        assert_ne!(a.content, a.header, "separate keys per purpose");
        assert_eq!(format!("{a:?}"), "Keys(<hidden>)");
    }

    /// Regression vector: today's output, hard-coded. If this changes, links made by
    /// older builds stop opening. (The primitives have their own official vectors
    /// upstream; this pins how Fuselane combines them.)
    #[test]
    fn the_content_format_is_pinned() {
        let k = Keys::derive(&[0; 32]);
        let mut first = [0u8; 16];
        k.apply(0, &mut first);
        assert_eq!(first, PINNED);
    }

    #[test]
    fn names_that_could_escape_the_folder_are_refused() {
        for bad in [
            "",
            ".",
            "..",
            "../x",
            "a/b",
            r"a\b",
            "C:x",
            "x\0y",
            "tab\there",
            " lead",
            "trail ",
            "dot.",
            "CON",
            "con.txt",
            "Com1",
            "LPT9.log",
            "a?b",
            "a*b",
            "a|b",
            "a\"b",
            "a<b",
        ] {
            assert!(
                matches!(check_name(bad), Err(CryptError::BadName(_))),
                "{bad:?}"
            );
        }
        assert!(check_name(&"x".repeat(MAX_NAME + 1)).is_err());
        for good in [
            "video.mp4",
            "holiday video.mp4",
            "COM0.txt",
            "console.log",
            ".bashrc",
            "नमस्ते.pdf",
            "a..b",
        ] {
            assert_eq!(check_name(good), Ok(()), "{good:?}");
        }
        let k = Keys::derive(&LINK_KEY);
        let evil = Header {
            name: "../../.ssh/authorized_keys".into(),
            ..header()
        };
        assert!(matches!(k.seal(&evil), Err(CryptError::BadName(_))));
    }

    #[test]
    fn the_plaintext_hash_detects_a_wrong_result() {
        let mut h = hasher();
        h.update(b"plain");
        h.update(b"text");
        assert_eq!(h.finalize().as_bytes(), &header().hash);
        assert_ne!(blake3::hash(b"plaintexT").as_bytes(), &header().hash);
    }
}
