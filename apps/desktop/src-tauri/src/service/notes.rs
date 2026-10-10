//! What a download's networks went through, for its detail (STEPS 8.2, 8.4):
//! a network benched as throttled (and back again), or one that can't connect
//! for a reason worth showing, such as its proxy turning down the login.
//!
//! One note per network, in the order they first had something to say. The
//! window writes the words, so speeds follow the person's unit choice and the
//! network goes by the name they gave it.

use std::collections::{HashMap, HashSet};

use fuselane_engine_http::download::LaneEvent;
use serde::Serialize;

/// One network's latest news in a download.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetNote {
    /// Device name; the window shows the person's name for it.
    pub name: String,
    /// `slow`: benched as throttled, the others carry the rest; `back`: a check
    /// found it fast again; `trouble`: it can't connect, `message` says why.
    pub kind: &'static str,
    /// Bytes per second: its throttled speed (slow) or the check's (back).
    pub rate: Option<f64>,
    /// Its best speed before it slowed down, when it had one.
    pub best: Option<f64>,
    /// Why it can't connect and what to do, in plain words (trouble).
    pub message: Option<String>,
}

/// The notes of one download's run.
#[derive(Debug, Default)]
pub struct Notes {
    notes: Vec<NetNote>,
    /// Lanes of each network that are benched (a network has one lane per
    /// server: the main one and each mirror).
    benched: HashMap<String, HashSet<u32>>,
}

impl Notes {
    /// Takes in an engine event; true when the notes changed.
    pub fn hear(&mut self, e: &LaneEvent) -> bool {
        let note = match e {
            LaneEvent::Throttled {
                net,
                name,
                rate,
                best,
            } => {
                self.benched.entry(name.clone()).or_default().insert(*net);
                NetNote {
                    name: name.clone(),
                    kind: "slow",
                    rate: Some(*rate),
                    best: (*best > 0.0).then_some(*best),
                    message: None,
                }
            }
            LaneEvent::Restored { net, name, rate } => {
                let lanes = self.benched.entry(name.clone()).or_default();
                lanes.remove(net);
                if !lanes.is_empty() {
                    return false; // still slow on another server
                }
                NetNote {
                    name: name.clone(),
                    kind: "back",
                    rate: Some(*rate),
                    best: None,
                    message: None,
                }
            }
            LaneEvent::Trouble { name, message, .. } => NetNote {
                name: name.clone(),
                kind: "trouble",
                rate: None,
                best: None,
                message: Some(message.clone()),
            },
        };
        match self.notes.iter_mut().find(|n| n.name == note.name) {
            Some(old) if *old == note => false,
            Some(old) => {
                *old = note;
                true
            }
            None => {
                self.notes.push(note);
                true
            }
        }
    }

    pub fn list(&self) -> Vec<NetNote> {
        self.notes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slow(net: u32, name: &str, rate: f64) -> LaneEvent {
        LaneEvent::Throttled {
            net,
            name: name.into(),
            rate,
            best: 1_500_000.0,
        }
    }

    #[test]
    fn one_note_per_network_with_its_latest_news() {
        let mut n = Notes::default();
        assert!(n.hear(&slow(2, "en5", 8_000.0)));
        assert!(!n.hear(&slow(2, "en5", 8_000.0)), "nothing new");
        assert!(n.hear(&LaneEvent::Trouble {
            net: 1,
            name: "en0".into(),
            message: "Wi-Fi's proxy turned down the username and password.".into(),
            lasting: true,
        }));
        assert!(n.hear(&LaneEvent::Restored {
            net: 2,
            name: "en5".into(),
            rate: 900_000.0,
        }));
        let list = n.list();
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].name.as_str(), list[0].kind), ("en5", "back"));
        assert_eq!(list[0].rate, Some(900_000.0));
        assert_eq!((list[1].name.as_str(), list[1].kind), ("en0", "trouble"));
    }

    #[test]
    fn a_network_is_back_only_when_every_lane_is() {
        let mut n = Notes::default();
        n.hear(&slow(2, "en5", 8_000.0));
        n.hear(&slow(102, "en5", 7_000.0)); // its lane to a mirror
        assert!(!n.hear(&LaneEvent::Restored {
            net: 2,
            name: "en5".into(),
            rate: 900_000.0,
        }));
        assert_eq!(n.list()[0].kind, "slow");
        assert!(n.hear(&LaneEvent::Restored {
            net: 102,
            name: "en5".into(),
            rate: 800_000.0,
        }));
        assert_eq!(n.list()[0].kind, "back");
    }

    #[test]
    fn a_download_shows_its_latest_runs_notes() {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("fuselane.db")).unwrap();
        let id = store
            .create("https://example.com/big.iso", dir.path())
            .unwrap();
        let svc = super::super::Service::new(store, dir.path().to_path_buf()).unwrap();
        let notes: std::sync::Arc<std::sync::Mutex<Notes>> = std::sync::Arc::default();
        super::super::lock(&svc.net_notes).insert(id, notes.clone());
        super::super::lock(&notes).hear(&slow(2, "en5", 8_000.0));
        let job = svc
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == id)
            .unwrap();
        assert_eq!(job.network_notes.len(), 1);
        assert_eq!(job.network_notes[0].kind, "slow");
        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["networkNotes"][0]["name"], "en5");
    }

    #[test]
    fn no_best_speed_is_shown_when_it_never_had_one() {
        let mut n = Notes::default();
        n.hear(&LaneEvent::Throttled {
            net: 2,
            name: "en5".into(),
            rate: 8_000.0,
            best: 0.0,
        });
        assert_eq!(n.list()[0].best, None);
    }
}
