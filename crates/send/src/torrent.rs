//! The torrent for a share, built over the encrypted view (librqbit's own builder
//! only hashes files as they are on disk).
//!
//! One file, a fixed generic name (the real name is inside the encrypted header),
//! no trackers in the info dict, not private, so the DHT can find it.

use std::io;

use crate::view::View;

/// Name every share's torrent uses, so it reveals nothing.
pub const NAME: &str = "fuselane-send";

/// Pieces between 256 KiB and 16 MiB, aiming for at most about 2,000 of them.
pub fn piece_length(len: u64) -> u32 {
    let wanted = len.div_ceil(2000).max(256 * 1024);
    wanted.next_power_of_two().min(16 * 1024 * 1024) as u32
}

/// The bencoded info dictionary and its SHA-1, the info-hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub info: Vec<u8>,
    pub info_hash: [u8; 20],
    pub piece_length: u32,
    pub pieces: usize,
}

impl Built {
    /// A complete .torrent: `d4:info<info>e`.
    pub fn torrent(&self) -> Vec<u8> {
        let mut t = b"d4:info".to_vec();
        t.extend_from_slice(&self.info);
        t.push(b'e');
        t
    }
}

fn sha1(data: &[u8]) -> [u8; 20] {
    use sha1::Digest;
    sha1::Sha1::digest(data).into()
}

/// Hashes the whole view piece by piece (one pass over the file).
pub fn build(view: &View, mut progress: impl FnMut(u64)) -> io::Result<Built> {
    let len = view.len();
    let piece = piece_length(len);
    let mut pieces = Vec::with_capacity((len.div_ceil(piece as u64) as usize) * 20);
    let mut buf = vec![0u8; piece as usize];
    let mut at = 0u64;
    while at < len {
        let n = (len - at).min(piece as u64) as usize;
        view.read(at, &mut buf[..n])?;
        pieces.extend_from_slice(&sha1(&buf[..n]));
        at += n as u64;
        progress(at);
    }
    // Keys in byte order, as bencoding requires: length < name < piece length < pieces.
    let mut info = format!(
        "d6:lengthi{len}e4:name{}:{NAME}12:piece lengthi{piece}e6:pieces{}:",
        NAME.len(),
        pieces.len()
    )
    .into_bytes();
    info.extend_from_slice(&pieces);
    info.push(b'e');
    Ok(Built {
        info_hash: sha1(&info),
        piece_length: piece,
        pieces: pieces.len() / 20,
        info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::{HEADER_BLOCK, Keys};
    use crate::view::ReadOnly;
    use crate::view::tests::{Mem, plain};

    fn view(size: usize) -> View {
        View::new(
            Keys::derive(&[5; 32]),
            Box::new(ReadOnly(Mem::with(vec![7; HEADER_BLOCK]))),
            Box::new(ReadOnly(Mem::with(plain(size)))),
            size as u64,
        )
    }

    #[test]
    fn piece_sizes_stay_sensible_from_tiny_to_huge() {
        assert_eq!(piece_length(1), 256 * 1024);
        assert_eq!(piece_length(500 * 1024 * 1024), 256 * 1024);
        assert_eq!(piece_length(4 * 1024 * 1024 * 1024), 4 * 1024 * 1024);
        assert_eq!(
            piece_length(200 * 1024 * 1024 * 1024),
            16 * 1024 * 1024,
            "capped"
        );
        for len in [1u64, 1 << 20, 1 << 30, 1 << 40] {
            assert!(piece_length(len).is_power_of_two());
        }
    }

    /// librqbit must read our torrent exactly as we meant it, including the
    /// info-hash peers will look up in the DHT.
    #[test]
    fn librqbit_parses_it_and_agrees_on_the_info_hash() {
        let size = 700_000;
        let v = view(size);
        let mut seen = 0;
        let b = build(&v, |at| seen = at).unwrap();
        assert_eq!(seen, v.len(), "progress reaches the end");
        let bytes = b.torrent();
        let t = librqbit::torrent_from_bytes(&bytes).unwrap();
        assert_eq!(t.info_hash.0, b.info_hash);
        let info = &t.info.data;
        assert_eq!(info.length, Some(v.len()));
        assert_eq!(info.piece_length, b.piece_length);
        assert_eq!(
            info.name.as_ref().map(|n| n.as_ref()),
            Some(NAME.as_bytes())
        );
        assert!(!info.private);
        assert_eq!(
            b.pieces,
            (v.len() as usize).div_ceil(b.piece_length as usize)
        );
    }

    #[test]
    fn piece_hashes_cover_the_ciphertext() {
        let v = view(300_000);
        let b = build(&v, |_| {}).unwrap();
        let mut first = vec![0u8; b.piece_length as usize];
        v.read(0, &mut first).unwrap();
        let start = b.info.windows(8).position(|w| w == b"6:pieces").unwrap() + 8;
        let colon = start + b.info[start..].iter().position(|&c| c == b':').unwrap();
        assert_eq!(&b.info[colon + 1..colon + 21], &sha1(&first));
    }

    #[test]
    fn the_same_share_always_builds_the_same_torrent() {
        assert_eq!(
            build(&view(1000), |_| {}).unwrap(),
            build(&view(1000), |_| {}).unwrap()
        );
        let other = View::new(
            Keys::derive(&[6; 32]),
            Box::new(ReadOnly(Mem::with(vec![7; HEADER_BLOCK]))),
            Box::new(ReadOnly(Mem::with(plain(1000)))),
            1000,
        );
        assert_ne!(
            build(&view(1000), |_| {}).unwrap().info_hash,
            build(&other, |_| {}).unwrap().info_hash,
            "a different key is a different torrent"
        );
    }

    #[test]
    fn an_unreadable_file_is_an_error_not_a_bad_torrent() {
        let v = View::new(
            Keys::derive(&[5; 32]),
            Box::new(ReadOnly(Mem::with(vec![7; HEADER_BLOCK]))),
            Box::new(ReadOnly(Mem::with(plain(10)))),
            1000, // claims more than the file has
        );
        assert!(build(&v, |_| {}).is_err());
    }
}
