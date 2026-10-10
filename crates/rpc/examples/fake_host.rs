//! Serves a fake download list on 127.0.0.1:6800 (secret "fuselane-demo") for
//! trying aria2 front ends such as AriaNg against the endpoint.

#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use fuselane_rpc::{Add, Host, Job, Server, Status};

struct Fake(Mutex<Vec<Job>>);

fn job(id: i64, name: &str, status: Status, done: u64, speed: u64) -> Job {
    Job {
        id,
        url: format!("https://example.com/{name}"),
        mirrors: vec![],
        dir: "/Users/me/Downloads".into(),
        name: name.into(),
        status,
        total: Some(4_700_000_000),
        done,
        speed,
        error: (status == Status::Error).then(|| "The server stopped answering.".into()),
    }
}

impl Host for Fake {
    fn jobs(&self) -> Vec<Job> {
        self.0.lock().unwrap().clone()
    }
    fn add(&self, add: Add) -> Result<i64, String> {
        eprintln!("add {add:?}");
        let mut j = self.0.lock().unwrap();
        let id = j.len() as i64 + 1;
        let name = add
            .out
            .unwrap_or_else(|| add.uris[0].rsplit('/').next().unwrap_or("file").to_string());
        j.push(job(id, &name, Status::Waiting, 0, 0));
        Ok(id)
    }
    fn pause(&self, id: i64) -> Result<(), String> {
        eprintln!("pause {id}");
        self.set(id, Status::Paused);
        Ok(())
    }
    fn resume(&self, id: i64) -> Result<(), String> {
        eprintln!("resume {id}");
        self.set(id, Status::Active);
        Ok(())
    }
    fn remove(&self, id: i64) -> Result<(), String> {
        eprintln!("remove {id}");
        self.0.lock().unwrap().retain(|j| j.id != id);
        Ok(())
    }
    fn default_dir(&self) -> String {
        "/Users/me/Downloads".into()
    }
    fn max_running(&self) -> usize {
        3
    }
    fn set_max_running(&self, _: usize) -> Result<(), String> {
        Ok(())
    }
}

impl Fake {
    fn set(&self, id: i64, s: Status) {
        for j in self.0.lock().unwrap().iter_mut() {
            if j.id == id {
                j.status = s;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let host = Arc::new(Fake(Mutex::new(vec![
        job(
            1,
            "ubuntu-26.04-desktop-amd64.iso",
            Status::Active,
            1_900_000_000,
            38_000_000,
        ),
        job(2, "fedora-43.iso", Status::Paused, 700_000_000, 0),
        job(3, "nightly-build.zip", Status::Error, 12_000_000, 0),
        job(4, "debian-13.iso", Status::Complete, 4_700_000_000, 0),
    ])));
    let s = Server::start(
        host,
        "127.0.0.1:6800".parse().unwrap(),
        "fuselane-demo".into(),
    )
    .await
    .unwrap();
    eprintln!("listening on {}", s.addr);
    std::future::pending::<()>().await;
}
