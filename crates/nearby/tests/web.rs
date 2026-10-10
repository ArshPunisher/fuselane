//! The phone page: token, uploads, offers, and what a browser would do.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fuselane_nearby::web::{Offer, PageHost, PhonePage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Default)]
struct Log(Mutex<Vec<Result<PathBuf, String>>>);

impl PageHost for Log {
    fn receiving(&self, _: &str, _: &str, _: u64) {}
    fn progress(&self, _: &str, _: u64) {}
    fn received(&self, _: &str, r: Result<PathBuf, String>) {
        self.0.lock().unwrap().push(r);
    }
}

async fn http(page: &PhonePage, head: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", page.addr.port()))
        .await
        .unwrap();
    s.write_all(head.as_bytes()).await.unwrap();
    s.write_all(body).await.unwrap();
    let mut out = vec![];
    s.read_to_end(&mut out).await.unwrap();
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let code = String::from_utf8_lossy(&out[9..12]).parse().unwrap();
    (code, out[split + 4..].to_vec())
}

fn get(path: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
}

#[tokio::test]
async fn the_page_needs_its_token_and_files_go_both_ways() {
    let inbox = tempfile::tempdir().unwrap();
    let log = Arc::new(Log::default());
    let page = PhonePage::start(
        log.clone(),
        inbox.path().to_path_buf(),
        "Arsh's <MacBook>".into(),
    )
    .await
    .unwrap();
    let base = format!("/p/{}", page.token);
    assert!(page.url("192.168.1.24".parse().unwrap()).ends_with(&base));

    // Without the token there's nothing.
    assert_eq!(http(&page, &get("/"), b"").await.0, 404);
    assert_eq!(http(&page, &get("/p/wrong"), b"").await.0, 404);
    let (code, html) = http(&page, &get(&base), b"").await;
    let html = String::from_utf8(html).unwrap();
    assert_eq!(code, 200);
    assert!(
        html.contains("Arsh&#39;s &lt;MacBook&gt;"),
        "the name is escaped"
    );

    // An upload lands under a free name.
    std::fs::write(inbox.path().join("photo.jpg"), b"old").unwrap();
    let data = vec![42u8; 700_000];
    let head = format!(
        "POST {base}/upload?name=photo.jpg HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        data.len()
    );
    assert_eq!(http(&page, &head, &data).await.0, 200);
    assert_eq!(
        std::fs::read(inbox.path().join("photo (2).jpg")).unwrap(),
        data
    );
    assert_eq!(
        std::fs::read(inbox.path().join("photo.jpg")).unwrap(),
        b"old"
    );

    // A short upload or a bad name leaves nothing behind.
    let head = format!(
        "POST {base}/upload?name=cut.bin HTTP/1.1\r\nHost: x\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n"
    );
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", page.addr.port()))
        .await
        .unwrap();
    s.write_all(head.as_bytes()).await.unwrap();
    s.write_all(&[1u8; 10]).await.unwrap();
    drop(s);
    let bad = format!(
        "POST {base}/upload?name=..%2F.ssh HTTP/1.1\r\nHost: x\r\nContent-Length: 1\r\nConnection: close\r\n\r\n"
    );
    assert_eq!(http(&page, &bad, b"x").await.0, 400);
    for _ in 0..100 {
        if log.0.lock().unwrap().len() >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let names: Vec<String> = std::fs::read_dir(inbox.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names
            .iter()
            .all(|n| !n.contains("fuselane-part") && n != "cut.bin"),
        "{names:?}"
    );

    // Offered files are listed and download exactly.
    let out = tempfile::tempdir().unwrap();
    let file = out.path().join("Boarding pass.pdf");
    std::fs::write(&file, b"%PDF-1.7 boarding").unwrap();
    page.offer(vec![Offer {
        id: "o1".into(),
        path: file,
        name: "Boarding pass.pdf".into(),
        size: 17,
    }]);
    let (_, list) = http(&page, &get(&format!("{base}/files")), b"").await;
    assert!(
        String::from_utf8(list)
            .unwrap()
            .contains("Boarding pass.pdf")
    );
    let (code, body) = http(&page, &get(&format!("{base}/file/o1")), b"").await;
    assert_eq!((code, &body[..]), (200, &b"%PDF-1.7 boarding"[..]));
    assert_eq!(
        http(&page, &get(&format!("{base}/file/nope")), b"").await.0,
        404
    );
}

#[derive(Default)]
struct Texts(Mutex<Vec<String>>);

impl PageHost for Texts {
    fn receiving(&self, _: &str, _: &str, _: u64) {}
    fn progress(&self, _: &str, _: u64) {}
    fn received(&self, _: &str, _: Result<PathBuf, String>) {}
    fn text(&self, text: &str) {
        self.0.lock().unwrap().push(text.to_string());
    }
}

fn post(path: &str, len: usize) -> String {
    format!("POST {path} HTTP/1.1\r\nHost: x\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n")
}

#[tokio::test]
async fn text_goes_both_ways_and_stays_small() {
    let inbox = tempfile::tempdir().unwrap();
    let texts = Arc::new(Texts::default());
    let page = PhonePage::start(texts.clone(), inbox.path().to_path_buf(), "Mac".into())
        .await
        .unwrap();
    let base = format!("/p/{}", page.token);

    // Phone to computer.
    let msg = "https://example.com/a?b=1 ✓";
    let (code, _) = http(
        &page,
        &post(&format!("{base}/text"), msg.len()),
        msg.as_bytes(),
    )
    .await;
    assert_eq!(code, 200);
    assert_eq!(texts.0.lock().unwrap().as_slice(), [msg.to_string()]);

    // Blank, not UTF-8, too long, or without the token: refused, with a reason.
    assert_eq!(
        http(&page, &post(&format!("{base}/text"), 3), b"  \n")
            .await
            .0,
        400
    );
    assert_eq!(
        http(&page, &post(&format!("{base}/text"), 2), &[0xff, 0xfe])
            .await
            .0,
        400
    );
    let big = vec![b'a'; 64 * 1024 + 1];
    let (code, why) = http(&page, &post(&format!("{base}/text"), big.len()), &big).await;
    assert_eq!(code, 413);
    assert!(String::from_utf8(why).unwrap().contains("64 KB"));
    // (No body: one the server never reads can reset the socket before the reply.)
    assert_eq!(http(&page, &post("/p/wrong/text", 0), b"").await.0, 404);
    assert_eq!(texts.0.lock().unwrap().len(), 1);

    // Computer to phone: the number changes with each offer.
    let read = |b: Vec<u8>| serde_json::from_slice::<serde_json::Value>(&b).unwrap();
    let first = read(http(&page, &get(&format!("{base}/text")), b"").await.1);
    assert_eq!(first["text"], serde_json::Value::Null);
    page.offer_text(Some("wifi password: hunter2".into()));
    let second = read(http(&page, &get(&format!("{base}/text")), b"").await.1);
    assert_eq!(second["text"], "wifi password: hunter2");
    assert_ne!(first["n"], second["n"]);
    page.offer_text(None);
    let third = read(http(&page, &get(&format!("{base}/text")), b"").await.1);
    assert_eq!(third["text"], serde_json::Value::Null);
}
