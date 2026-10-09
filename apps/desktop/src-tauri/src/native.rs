//! What the OS shell shows for downloads: notifications when one finishes or stops,
//! the combined progress on the Dock or taskbar icon, and the tray tooltip.
//! Pure decisions here; `main.rs` hands them to Tauri.

use std::collections::HashMap;

use crate::service::{JobView, Live};

/// A notification to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

/// Remembers each job's last status, to notice transitions.
#[derive(Debug, Default)]
pub struct Watcher {
    last: HashMap<i64, &'static str>,
    primed: bool,
    live: HashMap<i64, (u64, Option<u64>, f64)>,
}

impl Watcher {
    /// Notices for jobs that just finished or stopped. The first list only records
    /// statuses, so launching the app doesn't replay old news.
    pub fn jobs(&mut self, jobs: &[JobView]) -> Vec<Notice> {
        let mut out = Vec::new();
        for j in jobs {
            let before = self.last.insert(j.id, j.status);
            if !self.primed || before != Some("running") {
                continue;
            }
            match j.status {
                "completed" => out.push(Notice {
                    title: "Download finished".into(),
                    body: j.name.clone(),
                }),
                "failed" | "failed-final" => out.push(Notice {
                    title: format!("{} stopped", j.name),
                    body: j
                        .error
                        .clone()
                        .unwrap_or_else(|| "The download stopped.".into()),
                }),
                _ => {}
            }
        }
        let running: Vec<i64> = jobs
            .iter()
            .filter(|j| j.status == "running")
            .map(|j| j.id)
            .collect();
        self.live.retain(|id, _| running.contains(id));
        self.last.retain(|id, _| jobs.iter().any(|j| j.id == *id));
        self.primed = true;
        out
    }

    pub fn live(&mut self, l: &Live) {
        if self.last.get(&l.id) == Some(&"running") || !self.primed {
            self.live.insert(l.id, (l.written, l.total, l.rate));
        }
    }

    /// Combined progress of running downloads, 0..=100; `None` when nothing runs
    /// or no running download knows its size.
    pub fn progress(&self) -> Option<u64> {
        let (mut done, mut total) = (0u64, 0u64);
        for (written, size, _) in self.live.values() {
            if let Some(s) = size {
                done = done.saturating_add((*written).min(*s));
                total = total.saturating_add(*s);
            }
        }
        (total > 0).then(|| (done.saturating_mul(100) / total).min(100))
    }

    /// Tray tooltip: the combined speed while something runs.
    pub fn tooltip(&self) -> String {
        let speed: f64 = self
            .live
            .values()
            .map(|(_, _, r)| r)
            .filter(|r| r.is_finite())
            .sum();
        let n = self.live.len();
        match n {
            0 => "Fuselane".into(),
            1 => format!("Fuselane: {}/s", human(speed)),
            _ => format!("Fuselane: {n} downloads, {}/s", human(speed)),
        }
    }
}

fn human(bytes: f64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes.max(0.0);
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0} {}", U[i])
    } else {
        format!("{v:.1} {}", U[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: i64, status: &'static str) -> JobView {
        JobView {
            id,
            url: "u".into(),
            name: format!("file{id}.iso"),
            dir: "d".into(),
            status,
            resumable: false,
            written: 0,
            total: None,
            error: (status == "failed").then(|| "The server refused access.".into()),
            error_action: None,
            final_path: None,
            created_at: 0,
            position: 0,
            verify: false,
            speed_limit: 0,
            retry_in: None,
            start_at: None,
        }
    }

    fn live(id: i64, written: u64, total: Option<u64>, rate: f64) -> Live {
        Live {
            id,
            written,
            total,
            rate,
            networks: vec![],
            ticks: vec![],
            retries: 0,
            hedges: 0,
        }
    }

    #[test]
    fn launching_does_not_replay_old_news() {
        let mut w = Watcher::default();
        assert!(w.jobs(&[job(1, "completed"), job(2, "failed")]).is_empty());
    }

    #[test]
    fn finishing_and_stopping_are_announced_once() {
        let mut w = Watcher::default();
        w.jobs(&[job(1, "running"), job(2, "running"), job(3, "paused")]);
        let n = w.jobs(&[job(1, "completed"), job(2, "failed"), job(3, "completed")]);
        assert_eq!(
            n.len(),
            2,
            "a paused job finishing elsewhere isn't news: {n:?}"
        );
        assert_eq!(
            n[0],
            Notice {
                title: "Download finished".into(),
                body: "file1.iso".into()
            }
        );
        assert_eq!(n[1].title, "file2.iso stopped");
        assert_eq!(n[1].body, "The server refused access.");
        assert!(
            w.jobs(&[job(1, "completed"), job(2, "failed")]).is_empty(),
            "announced twice"
        );
        // Pausing is the user's own action: no notification.
        w.jobs(&[job(4, "running")]);
        assert!(w.jobs(&[job(4, "paused")]).is_empty());
    }

    #[test]
    fn progress_combines_running_downloads_and_ignores_unknown_sizes() {
        let mut w = Watcher::default();
        assert_eq!(w.progress(), None);
        w.jobs(&[job(1, "running"), job(2, "running"), job(3, "running")]);
        w.live(&live(1, 50, Some(100), 1024.0));
        w.live(&live(2, 150, Some(300), 2048.0));
        w.live(&live(3, 999, None, 0.0));
        assert_eq!(w.progress(), Some(50));
        assert_eq!(w.tooltip(), "Fuselane: 3 downloads, 3.0 KB/s");
        // A lying total never shows more than 100%.
        w.live(&live(1, 500, Some(100), 0.0));
        assert!(w.progress().unwrap() <= 100);
        // Finished jobs drop out; nothing running clears the bar.
        w.jobs(&[job(1, "completed"), job(2, "completed"), job(3, "paused")]);
        assert_eq!(w.progress(), None);
        assert_eq!(w.tooltip(), "Fuselane");
    }

    #[test]
    fn hostile_numbers_dont_panic() {
        let mut w = Watcher::default();
        w.jobs(&[job(1, "running"), job(2, "running")]);
        w.live(&live(1, u64::MAX, Some(u64::MAX), f64::NAN));
        w.live(&live(2, u64::MAX, Some(u64::MAX), f64::INFINITY));
        let _ = w.progress();
        assert!(w.tooltip().starts_with("Fuselane"));
    }
}
