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
        vec![
            "amber".into(),
            "river".into(),
            "candle".into(),
            "orbit".into(),
        ],
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
    assert!(html.contains("<li>orbit</li>"));

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
