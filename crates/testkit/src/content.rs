//! Deterministic file content of any size, generated on the fly, so tests can
//! serve multi-gigabyte files without storing them and check every byte.

use sha2::{Digest, Sha256};

/// A virtual file: `size` bytes whose value at offset `i` depends only on `seed` and `i`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Content {
    pub size: u64,
    pub seed: u64,
}

impl Content {
    pub fn new(size: u64, seed: u64) -> Content {
        Content { size, seed }
    }

    /// The byte at `offset` (splitmix64 of the 8-byte word, so neighbours differ).
    pub fn byte(&self, offset: u64) -> u8 {
        let word = offset / 8;
        let mut z = word
            .wrapping_add(self.seed)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> ((offset % 8) * 8)) as u8
    }

    /// Fills `buf` with the bytes starting at `offset`.
    pub fn fill(&self, offset: u64, buf: &mut [u8]) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = self.byte(offset + i as u64);
        }
    }

    /// SHA-256 of the whole content.
    pub fn sha256(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 1 << 16];
        let mut off = 0;
        while off < self.size {
            let n = ((self.size - off) as usize).min(buf.len());
            self.fill(off, &mut buf[..n]);
            h.update(&buf[..n]);
            off += n as u64;
        }
        h.finalize().into()
    }
}

/// SHA-256 of a file on disk.
pub fn sha256_file(path: &std::path::Path) -> std::io::Result<[u8; 32]> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_is_deterministic_and_varied() {
        let c = Content::new(1 << 20, 7);
        let mut a = vec![0u8; 4096];
        let mut b = vec![0u8; 4096];
        c.fill(12_345, &mut a);
        c.fill(12_345, &mut b);
        assert_eq!(a, b);
        let mut other = vec![0u8; 4096];
        Content::new(1 << 20, 8).fill(12_345, &mut other);
        assert_ne!(a, other, "different seeds give different files");
        let distinct: std::collections::HashSet<u8> = a.iter().copied().collect();
        assert!(distinct.len() > 200, "bytes should look random");
        assert_eq!(
            Content::new(0, 1).sha256(),
            <[u8; 32]>::from(Sha256::digest([]))
        );
    }

    #[test]
    fn offsets_past_four_gigabytes_work() {
        let c = Content::new(6 << 30, 1);
        let _ = c.byte((5 << 30) + 17);
    }
}
