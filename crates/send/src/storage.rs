//! librqbit storage backed by a share's `View` (STEPS 6.3): the engine reads and
//! writes the encrypted torrent while the disk holds only the plain file.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, bail};
use librqbit::storage::{BoxStorageFactory, StorageFactory, StorageFactoryExt, TorrentStorage};
use librqbit::{ManagedTorrentShared, TorrentMetadata};

use crate::view::View;

/// Hands librqbit storage for one share. Use it with exactly that share's torrent.
#[derive(Debug, Clone)]
pub struct ShareStorageFactory(pub Arc<View>);

impl StorageFactory for ShareStorageFactory {
    type Storage = ShareStorage;

    fn create(&self, _: &ManagedTorrentShared, _: &TorrentMetadata) -> anyhow::Result<ShareStorage> {
        Ok(ShareStorage(self.0.clone()))
    }

    fn clone_box(&self) -> BoxStorageFactory {
        self.clone().boxed()
    }
}

#[derive(Debug)]
pub struct ShareStorage(Arc<View>);

impl ShareStorage {
    fn one_file(file_id: usize) -> anyhow::Result<()> {
        if file_id != 0 {
            bail!("a share has one file, not file {file_id}");
        }
        Ok(())
    }
}

impl TorrentStorage for ShareStorage {
    fn init(&mut self, _: &ManagedTorrentShared, metadata: &TorrentMetadata) -> anyhow::Result<()> {
        let files: Vec<u64> = metadata.file_infos.iter().map(|f| f.len).collect();
        if files != [self.0.len()] {
            bail!(
                "this torrent doesn't match the share (files {files:?}, share {} bytes)",
                self.0.len()
            );
        }
        Ok(())
    }

    fn pread_exact(&self, file_id: usize, offset: u64, buf: &mut [u8]) -> anyhow::Result<()> {
        Self::one_file(file_id)?;
        self.0.read(offset, buf).context("reading the share")
    }

    fn pwrite_all(&self, file_id: usize, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        Self::one_file(file_id)?;
        self.0.write(offset, buf).context("writing the share")
    }

    /// Never deletes: the sender's file is theirs, and the app cleans up a
    /// cancelled receive itself (it knows which side files it made).
    fn remove_file(&self, _: usize, _: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove_directory_if_empty(&self, _: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn ensure_file_length(&self, file_id: usize, length: u64) -> anyhow::Result<()> {
        Self::one_file(file_id)?;
        self.0.set_len(length).context("sizing the share")
    }

    fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
        Ok(Box::new(ShareStorage(self.0.clone())))
    }
}
