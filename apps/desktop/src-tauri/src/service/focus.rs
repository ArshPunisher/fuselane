//! "Do this one now" (B9.1): one download gets every network to itself. The
//! others that were running pause and say why; nothing else starts. When the
//! focused one ends for any reason (done, paused, failed, removed), the ones it
//! held carry on by themselves.

use std::sync::Arc;

use fuselane_core::{Event, Status};

use super::{Service, UiError, lock, not_found, store_error};

impl Service {
    /// The download that has every network to itself, if any.
    pub fn focused(&self) -> Option<i64> {
        *lock(&self.focus)
    }

    /// Gives `id` every network now: it goes first, the others running pause
    /// until it ends. A stopped download is started.
    pub fn focus(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        if matches!(
            job.status,
            Status::Completed | Status::Failed { resumable: false }
        ) {
            return Err(UiError::new(
                "not-resumable",
                "This download can't run, so it can't go first.",
                Some("Download it again to start it from the beginning."),
            ));
        }
        let previous = lock(&self.focus).replace(id);
        if previous == Some(id) {
            return Ok(());
        }
        self.store.reorder(&[id]).map_err(store_error)?;
        let name = super::job_name(&job);
        {
            let mut held = lock(&self.focus_held);
            for (other, r) in lock(&self.running).iter() {
                if *other != id && held.insert(*other) {
                    r.cancel.cancel(); // its task records the pause
                }
            }
        }
        // Held downloads that already stopped say why too (running ones once paused).
        self.note_held(&name);
        if matches!(job.status, Status::Paused | Status::Failed { .. }) {
            self.resume(id)?;
        } else {
            self.publish_jobs();
            self.pump();
        }
        Ok(())
    }

    /// Ends "do this one now" by hand; the held downloads carry on.
    pub fn unfocus(self: &Arc<Self>) {
        if lock(&self.focus).take().is_some() {
            self.release_held();
        }
    }

    /// Called when a download's run ends: if it was the focused one, the rest go on.
    pub(super) fn focus_ended(self: &Arc<Self>, id: i64) {
        let mut focus = lock(&self.focus);
        if *focus != Some(id) {
            return;
        }
        // Still queued (a retry, a schedule pause), so it keeps going first.
        if self
            .store
            .get(id)
            .is_ok_and(|j| j.status == Status::Queued && !lock(&self.schedule_paused).contains(&id))
        {
            return;
        }
        *focus = None;
        drop(focus);
        self.release_held();
    }

    /// Whether the queue may start `id` now.
    pub(super) fn focus_allows(&self, id: i64) -> bool {
        lock(&self.focus).is_none_or(|f| f == id)
    }

    /// Marks a held download that has paused with the reason, once it has.
    pub(super) fn note_held(&self, focus_name: &str) {
        for id in lock(&self.focus_held).iter() {
            if self
                .store
                .get(*id)
                .is_ok_and(|j| j.status == Status::Paused)
            {
                let _ = self.store.set_error(
                    *id,
                    &format!("Waiting: {focus_name} goes first. This carries on after it."),
                    "focus",
                );
            }
        }
    }

    /// Someone paused a held download by hand: it stays paused afterwards.
    pub(super) fn forget_held(&self, id: i64) {
        lock(&self.focus_held).remove(&id);
    }

    fn release_held(self: &Arc<Self>) {
        let ids: Vec<i64> = lock(&self.focus_held).drain().collect();
        for id in ids {
            if self.store.get(id).is_ok_and(|j| j.status == Status::Paused) {
                let _ = self.store.apply(id, Event::Resume, None);
            }
        }
        self.publish_jobs();
        self.pump();
    }
}
