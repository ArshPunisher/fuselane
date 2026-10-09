//! Checksums found by themselves (B9.7): before a download first starts,
//! Fuselane looks next to it for a published SHA-256 (`file.sha256`,
//! `SHA256SUMS`) and, when one lists this file, verifies the finished file
//! against it. On by default; a setting turns it off.

use fuselane_core::Job;

use super::{Service, UiError, store_error};

impl Service {
    pub fn find_checksums(&self) -> bool {
        self.find_checksums
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_find_checksums(&self, on: bool) -> Result<bool, UiError> {
        self.store
            .set_setting("find_checksums", if on { "true" } else { "false" })
            .map_err(store_error)?;
        self.find_checksums
            .store(on, std::sync::atomic::Ordering::Relaxed);
        Ok(on)
    }

    /// Looks once per download, before its first start, unless it already has a
    /// checksum, the setting is off, or it carries a browser sign-in (whose
    /// cookies are for the file only). Returns the job as it should run.
    pub(super) async fn with_found_checksum(&self, job: Job) -> Job {
        let signed_in = super::lock(&self.sessions).contains_key(&job.id);
        if job.expected_sha256.is_some()
            || job.sha256_from.is_some()
            || signed_in
            || !self.find_checksums()
        {
            return job;
        }
        let Some(file) = fuselane_core::checksums::file_of(&job.url) else {
            return job;
        };
        let found = fuselane_core::runner::find_checksum(&job.url, &file).await;
        let _ = self.store.set_found_sha256(
            job.id,
            found.as_ref().map(|(h, from)| (h.as_str(), from.as_str())),
        );
        self.store.get(job.id).unwrap_or(job)
    }
}
