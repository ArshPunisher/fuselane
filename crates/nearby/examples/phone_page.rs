//! Serves the phone page for a manual or browser check:
//! `cargo run -p fuselane-nearby --example phone_page -- <inbox folder> [file to offer]`
//! Prints the link; files sent from the page land in the inbox folder.

#![allow(clippy::unwrap_used, clippy::expect_used)] // a developer tool

use std::path::PathBuf;
use std::sync::Arc;

use fuselane_nearby::web::{Offer, PageHost, PhonePage};

struct Print;

impl PageHost for Print {
    fn receiving(&self, _: &str, name: &str, size: u64) {
        println!("receiving {name} ({size} bytes)");
    }
    fn progress(&self, _: &str, _: u64) {}
    fn received(&self, _: &str, r: Result<PathBuf, String>) {
        println!("received {r:?}");
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let inbox = PathBuf::from(args.next().expect("an inbox folder"));
    let page = PhonePage::start(Arc::new(Print), inbox, "Fuselane test".into())
        .await
        .unwrap();
    if let Some(f) = args.next() {
        let path = PathBuf::from(f);
        let size = std::fs::metadata(&path).unwrap().len();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        page.offer(vec![Offer {
            id: "o1".into(),
            path,
            name,
            size,
        }]);
    }
    println!("{}", page.url(std::net::Ipv4Addr::LOCALHOST));
    tokio::signal::ctrl_c().await.unwrap();
}
