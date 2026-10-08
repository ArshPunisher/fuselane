//! What the torrent sees versus what is on disk (FUSE-SEND.md §4).
//!
//! The torrent's single file is `[header block][encrypted content]`. On disk there
//! is only the plain file (and, while receiving, the header block in a small side
//! file). Reads encrypt and writes decrypt, so piece hashes always cover ciphertext
//! and no second copy of a large file is ever made.

use std::io;

use crate::crypt::{HEADER_BLOCK, Keys};

/// Positioned reads and writes, so pieces can be handled in parallel.
pub trait Disk: Send + Sync {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
    fn write_at(&self, offset: u64, buf: &[u8]) -> io::Result<()>;
    /// Makes room for the whole file up front (receiving).
    fn set_len(&self, len: u64) -> io::Result<()>;
}

impl Disk for std::fs::File {
    fn set_len(&self, len: u64) -> io::Result<()> {
        std::fs::File::set_len(self, len)
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::FileExt::read_exact_at(self, buf, offset)
        }
        #[cfg(windows)]
        {
            let (mut done, mut buf) = (0u64, buf);
            while !buf.is_empty() {
                match std::os::windows::fs::FileExt::seek_read(self, buf, offset + done)? {
                    0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                    n => {
                        buf = &mut buf[n..];
                        done += n as u64;
                    }
                }
            }
            Ok(())
        }
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::FileExt::write_all_at(self, buf, offset)
        }
        #[cfg(windows)]
        {
            let (mut done, mut buf) = (0u64, buf);
            while !buf.is_empty() {
                match std::os::windows::fs::FileExt::seek_write(self, buf, offset + done)? {
                    0 => return Err(io::ErrorKind::WriteZero.into()),
                    n => {
                        buf = &buf[n..];
                        done += n as u64;
                    }
                }
            }
            Ok(())
        }
    }
}

/// A file the sender shares: never written to.
#[derive(Debug)]
pub struct ReadOnly<D>(pub D);

impl<D: Disk> Disk for ReadOnly<D> {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_at(offset, buf)
    }
    fn write_at(&self, _: u64, _: &[u8]) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "a shared file is never written to",
        ))
    }
    /// The shared file already has its size; it is never resized.
    fn set_len(&self, _: u64) -> io::Result<()> {
        Ok(())
    }
}

/// The torrent's view of one share.
pub struct View {
    keys: Keys,
    head: Box<dyn Disk>,
    body: Box<dyn Disk>,
    /// Plain content size.
    size: u64,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl View {
    pub fn new(keys: Keys, head: Box<dyn Disk>, body: Box<dyn Disk>, size: u64) -> View {
        View {
            keys,
            head,
            body,
            size,
        }
    }

    /// The torrent's file length.
    pub fn len(&self) -> u64 {
        HEADER_BLOCK as u64 + self.size
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    fn check(&self, offset: u64, len: usize) -> io::Result<()> {
        match offset.checked_add(len as u64) {
            Some(end) if end <= self.len() => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{len} bytes at {offset} are past the share's end ({})",
                    self.len()
                ),
            )),
        }
    }

    /// Splits `[offset, offset+len)` at the header/content edge.
    fn split(offset: u64, len: usize) -> (usize, u64) {
        let hb = HEADER_BLOCK as u64;
        let in_head = if offset < hb {
            (hb - offset).min(len as u64) as usize
        } else {
            0
        };
        (in_head, (offset + in_head as u64).saturating_sub(hb))
    }

    /// Gives both files their final sizes (a no-op for a shared file).
    pub fn set_len(&self, len: u64) -> io::Result<()> {
        if len != self.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the torrent says {len} bytes, the share has {}", self.len()),
            ));
        }
        self.head.set_len(HEADER_BLOCK as u64)?;
        self.body.set_len(self.size)
    }

    /// Ciphertext at `offset` in the torrent's file.
    pub fn read(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.check(offset, buf.len())?;
        let (in_head, body_at) = Self::split(offset, buf.len());
        let (head, body) = buf.split_at_mut(in_head);
        if !head.is_empty() {
            self.head.read_at(offset, head)?;
        }
        if !body.is_empty() {
            self.body.read_at(body_at, body)?;
            self.keys.apply(body_at, body);
        }
        Ok(())
    }

    /// Takes ciphertext at `offset` and stores it decrypted.
    pub fn write(&self, offset: u64, buf: &[u8]) -> io::Result<()> {
        self.check(offset, buf.len())?;
        let (in_head, body_at) = Self::split(offset, buf.len());
        let (head, body) = buf.split_at(in_head);
        if !head.is_empty() {
            self.head.write_at(offset, head)?;
        }
        if !body.is_empty() {
            let mut plain = body.to_vec();
            self.keys.apply(body_at, &mut plain);
            self.body.write_at(body_at, &plain)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A disk in memory, for tests.
    #[derive(Debug, Default)]
    pub struct Mem(pub Mutex<Vec<u8>>);

    impl Mem {
        pub fn with(bytes: Vec<u8>) -> Mem {
            Mem(Mutex::new(bytes))
        }
    }

    impl Disk for Mem {
        fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
            let v = self.0.lock().unwrap();
            let s = offset as usize;
            let src = v
                .get(s..s + buf.len())
                .ok_or(io::ErrorKind::UnexpectedEof)?;
            buf.copy_from_slice(src);
            Ok(())
        }
        fn write_at(&self, offset: u64, buf: &[u8]) -> io::Result<()> {
            let mut v = self.0.lock().unwrap();
            let end = offset as usize + buf.len();
            if v.len() < end {
                v.resize(end, 0);
            }
            v[offset as usize..end].copy_from_slice(buf);
            Ok(())
        }
        fn set_len(&self, len: u64) -> io::Result<()> {
            self.0.lock().unwrap().resize(len as usize, 0);
            Ok(())
        }
    }

    impl Disk for std::sync::Arc<Mem> {
        fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
            (**self).read_at(offset, buf)
        }
        fn write_at(&self, offset: u64, buf: &[u8]) -> io::Result<()> {
            (**self).write_at(offset, buf)
        }
        fn set_len(&self, len: u64) -> io::Result<()> {
            (**self).set_len(len)
        }
    }

    pub fn plain(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 + 7) as u8).collect()
    }

    const KEY: [u8; 32] = [9; 32];

    fn sender(content: Vec<u8>, head: Vec<u8>) -> View {
        let size = content.len() as u64;
        View::new(
            Keys::derive(&KEY),
            Box::new(ReadOnly(Mem::with(head))),
            Box::new(ReadOnly(Mem::with(content))),
            size,
        )
    }

    #[test]
    fn reads_across_the_header_edge_match_the_whole_view() {
        let head: Vec<u8> = (0..HEADER_BLOCK).map(|i| i as u8).collect();
        let content = plain(5000);
        let v = sender(content.clone(), head.clone());
        assert_eq!(v.len(), (HEADER_BLOCK + 5000) as u64);
        let mut whole = vec![0u8; v.len() as usize];
        v.read(0, &mut whole).unwrap();
        assert_eq!(&whole[..HEADER_BLOCK], &head[..]);
        let mut enc = content.clone();
        Keys::derive(&KEY).apply(0, &mut enc);
        assert_eq!(&whole[HEADER_BLOCK..], &enc[..], "content is encrypted");
        for (off, len) in [
            (0, 1),
            (1000, 100),
            (1023, 2),
            (1024, 1),
            (1020, 500),
            (6023, 1),
            (0, 6024),
        ] {
            let mut b = vec![0u8; len];
            v.read(off, &mut b).unwrap();
            assert_eq!(b, whole[off as usize..off as usize + len], "{off}+{len}");
        }
    }

    #[test]
    fn writes_from_any_offset_rebuild_the_plain_file() {
        let head: Vec<u8> = (0..HEADER_BLOCK).map(|i| (i * 3) as u8).collect();
        let content = plain(3000);
        let src = sender(content.clone(), head.clone());
        let mut whole = vec![0u8; src.len() as usize];
        src.read(0, &mut whole).unwrap();

        let (h, b) = (
            std::sync::Arc::new(Mem::default()),
            std::sync::Arc::new(Mem::default()),
        );
        let dst = View::new(
            Keys::derive(&KEY),
            Box::new(h.clone()),
            Box::new(b.clone()),
            3000,
        );
        // Out of order, straddling the edge, like pieces arriving from peers.
        for (off, len) in [(2000usize, 2024usize), (1000, 1000), (0, 1000)] {
            dst.write(off as u64, &whole[off..off + len]).unwrap();
        }
        assert_eq!(*h.0.lock().unwrap(), head);
        assert_eq!(*b.0.lock().unwrap(), content, "plain on disk");
        let mut back = vec![0u8; whole.len()];
        dst.read(0, &mut back).unwrap();
        assert_eq!(
            back, whole,
            "reading back gives the ciphertext the hashes cover"
        );
    }

    #[test]
    fn nothing_past_the_end_and_never_a_write_to_a_shared_file() {
        let v = sender(plain(10), vec![0; HEADER_BLOCK]);
        let mut b = [0u8; 2];
        assert!(v.read(v.len() - 1, &mut b).is_err());
        assert!(v.read(u64::MAX, &mut b).is_err(), "no overflow");
        assert_eq!(
            v.write(0, &[1]).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(v.write(HEADER_BLOCK as u64, &[1]).is_err());
    }

    #[test]
    fn real_files_read_and_write_at_offsets() {
        let d = tempfile::tempdir().unwrap();
        let f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(d.path().join("f"))
            .unwrap();
        f.write_at(5, b"world").unwrap();
        f.write_at(0, b"hello").unwrap();
        let mut b = [0u8; 10];
        f.read_at(0, &mut b).unwrap();
        assert_eq!(&b, b"helloworld");
        assert!(f.read_at(8, &mut b).is_err(), "short read is an error");
    }
}
