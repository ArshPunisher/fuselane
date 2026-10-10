//! The aria2-compatible endpoint, as aria2 tools would call it.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::sync::{Arc, Mutex};

use fuselane_rpc::{Add, Event, Host, Job, Rpc, Server, Status, gid};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Default)]
struct Fake {
    jobs: Mutex<Vec<Job>>,
    max: Mutex<usize>,
}

impl Fake {
    fn job(&self, id: i64) -> Job {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .find(|j| j.id == id)
            .unwrap()
            .clone()
    }
    fn set(&self, id: i64, status: Status) {
        for j in self.jobs.lock().unwrap().iter_mut() {
            if j.id == id {
                j.status = status;
            }
        }
    }
}

impl Host for Fake {
    fn jobs(&self) -> Vec<Job> {
        self.jobs.lock().unwrap().clone()
    }
    fn add(&self, add: Add) -> Result<i64, String> {
        if !add.uris[0].starts_with("http") {
            return Err("Links start with http:// or https://.".into());
        }
        let mut jobs = self.jobs.lock().unwrap();
        let id = jobs.len() as i64 + 10;
        jobs.push(Job {
            id,
            url: add.uris[0].clone(),
            mirrors: add.uris[1..].to_vec(),
            dir: add.dir.unwrap_or_else(|| "/Users/me/Downloads".into()),
            name: add.out.unwrap_or_else(|| "file.iso".into()),
            status: if add.paused {
                Status::Paused
            } else {
                Status::Active
            },
            total: Some(1000),
            done: 250,
            speed: 4096,
            error: None,
        });
        Ok(id)
    }
    fn add_metalink(
        &self,
        xml: &str,
        dir: Option<String>,
        paused: bool,
    ) -> Result<Vec<i64>, String> {
        if !xml.contains("<metalink") {
            return Err("This isn't a Metalink file.".into());
        }
        let a = self.add(Add {
            uris: vec!["https://one.example/a.iso".into()],
            dir: dir.clone(),
            out: Some("a.iso".into()),
            paused,
        })?;
        Ok(vec![a])
    }
    fn pause(&self, id: i64) -> Result<(), String> {
        self.set(id, Status::Paused);
        Ok(())
    }
    fn resume(&self, id: i64) -> Result<(), String> {
        self.set(id, Status::Waiting);
        Ok(())
    }
    fn remove(&self, id: i64) -> Result<(), String> {
        self.jobs.lock().unwrap().retain(|j| j.id != id);
        Ok(())
    }
    fn default_dir(&self) -> String {
        "/Users/me/Downloads".into()
    }
    fn max_running(&self) -> usize {
        *self.max.lock().unwrap()
    }
    fn set_max_running(&self, n: usize) -> Result<(), String> {
        if n == 0 || n > 16 {
            return Err("Between 1 and 16 at once.".into());
        }
        *self.max.lock().unwrap() = n;
        Ok(())
    }
}

const SECRET: &str = "s3cret-s3cret";

fn call(rpc: &Rpc<Fake>, method: &str, params: Value) -> (u16, Value) {
    let mut p = vec![json!(format!("token:{SECRET}"))];
    p.extend(params.as_array().cloned().unwrap_or_default());
    let body = json!({"jsonrpc": "2.0", "id": "q", "method": method, "params": p});
    let (code, v) = rpc.handle(body.to_string().as_bytes());
    (code.as_u16(), v)
}

fn ok(rpc: &Rpc<Fake>, method: &str, params: Value) -> Value {
    let (code, v) = call(rpc, method, params);
    assert_eq!(code, 200, "{method}: {v}");
    v["result"].clone()
}

#[test]
fn a_wrong_or_missing_secret_is_refused() {
    let rpc = Rpc::new(Arc::new(Fake::default()), SECRET.into());
    for params in [json!([]), json!(["token:nope"]), json!(["s3cret-s3cret"])] {
        let body =
            json!({"jsonrpc": "2.0", "id": 1, "method": "aria2.getVersion", "params": params});
        let (code, v) = rpc.handle(body.to_string().as_bytes());
        assert_eq!(code.as_u16(), 401);
        assert_eq!(v["error"]["message"], "Unauthorized");
    }
    // Like aria2, the method list needs no secret.
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "system.listMethods"});
    let (code, v) = rpc.handle(body.to_string().as_bytes());
    assert_eq!(code.as_u16(), 200);
    assert!(
        v["result"]
            .as_array()
            .unwrap()
            .contains(&json!("aria2.addUri"))
    );
    // Nor does the list of notifications a WebSocket can get.
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "system.listNotifications"});
    let (code, v) = rpc.handle(body.to_string().as_bytes());
    assert_eq!(code.as_u16(), 200);
    assert_eq!(
        v["result"],
        json!([
            "aria2.onDownloadStart",
            "aria2.onDownloadPause",
            "aria2.onDownloadStop",
            "aria2.onDownloadComplete",
            "aria2.onDownloadError"
        ])
    );
    // Not JSON, an unknown method: a reason each.
    assert_eq!(rpc.handle(b"{nope").0.as_u16(), 400);
    assert_eq!(call(&rpc, "aria2.addTorrent", json!([])).0, 404);
}

#[test]
fn add_watch_pause_and_remove_like_ariang_does() {
    let host = Arc::new(Fake::default());
    let rpc = Rpc::new(host.clone(), SECRET.into());
    let g = ok(
        &rpc,
        "aria2.addUri",
        json!([["https://a.example/f.iso", "https://b.example/f.iso"], {"dir": "/tmp/x", "out": "f.iso"}]),
    );
    assert_eq!(g, gid(10));
    let j = host.job(10);
    assert_eq!(
        (j.dir.as_str(), j.name.as_str(), j.mirrors.len()),
        ("/tmp/x", "f.iso", 1)
    );

    let s = ok(&rpc, "aria2.tellStatus", json!([g]));
    assert_eq!(s["status"], "active");
    assert_eq!(s["totalLength"], "1000");
    assert_eq!(s["completedLength"], "250");
    assert_eq!(s["downloadSpeed"], "4096");
    assert_eq!(s["files"][0]["path"], "/tmp/x/f.iso");
    assert_eq!(s["files"][0]["uris"][1]["uri"], "https://b.example/f.iso");
    // Only the keys asked for.
    let s = ok(&rpc, "aria2.tellStatus", json!([g, ["gid", "status"]]));
    assert_eq!(s.as_object().unwrap().len(), 2);

    assert_eq!(
        ok(&rpc, "aria2.tellActive", json!([]))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(ok(&rpc, "aria2.pause", json!([g])), g);
    assert_eq!(ok(&rpc, "aria2.tellActive", json!([])), json!([]));
    assert_eq!(
        ok(&rpc, "aria2.tellWaiting", json!([0, 10]))[0]["status"],
        "paused"
    );
    ok(&rpc, "aria2.unpause", json!([g]));
    assert_eq!(host.job(10).status, Status::Waiting);

    let stat = ok(&rpc, "aria2.getGlobalStat", json!([]));
    assert_eq!(stat["numWaiting"], "1");

    // A finished one shows under stopped and can be cleared.
    host.set(10, Status::Complete);
    assert_eq!(
        ok(&rpc, "aria2.tellStopped", json!([-1, 5]))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(ok(&rpc, "aria2.removeDownloadResult", json!([g])), "OK");
    assert!(host.jobs().is_empty());

    // Unknown GIDs and bad links come back as aria2-style errors.
    let (code, v) = call(&rpc, "aria2.tellStatus", json!(["00000000000000ff"]));
    assert_eq!(code, 400);
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not found")
    );
    let (code, v) = call(&rpc, "aria2.addUri", json!([["ftp://x/y"]]));
    assert_eq!(code, 400);
    assert!(v["error"]["message"].as_str().unwrap().contains("https://"));
    assert_eq!(call(&rpc, "aria2.addUri", json!([[]])).0, 400);
}

#[test]
fn metalinks_come_in_base64() {
    let host = Arc::new(Fake::default());
    let rpc = Rpc::new(host.clone(), SECRET.into());
    // "<metalink/>" in base64.
    let g = ok(
        &rpc,
        "aria2.addMetalink",
        json!(["PG1ldGFsaW5rLz4=", {"pause": "true"}]),
    );
    assert_eq!(g, json!([gid(10)]));
    assert_eq!(host.job(10).status, Status::Paused);
    let (code, v) = call(&rpc, "aria2.addMetalink", json!(["not base64!"]));
    assert_eq!(code, 400);
    assert!(v["error"]["message"].as_str().unwrap().contains("base64"));
    // "<rss/>": not a Metalink.
    let (code, v) = call(&rpc, "aria2.addMetalink", json!(["PHJzcy8+"]));
    assert_eq!(code, 400);
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("isn't a Metalink")
    );
}

#[test]
fn options_version_and_multicall() {
    let host = Arc::new(Fake::default());
    *host.max.lock().unwrap() = 3;
    let rpc = Rpc::new(host.clone(), SECRET.into());
    assert_eq!(ok(&rpc, "aria2.getVersion", json!([]))["version"], "1.37.0");
    let o = ok(&rpc, "aria2.getGlobalOption", json!([]));
    assert_eq!(o["max-concurrent-downloads"], "3");
    ok(
        &rpc,
        "aria2.changeGlobalOption",
        json!([{"max-concurrent-downloads": "5"}]),
    );
    assert_eq!(host.max_running(), 5);
    assert_eq!(
        call(
            &rpc,
            "aria2.changeGlobalOption",
            json!([{"max-concurrent-downloads": "99"}])
        )
        .0,
        400
    );

    // AriaNg polls with multicall; each call carries the secret.
    let t = json!(format!("token:{SECRET}"));
    let body = json!({"jsonrpc": "2.0", "id": 7, "method": "system.multicall", "params": [[
        {"methodName": "aria2.getGlobalStat", "params": [t]},
        {"methodName": "aria2.getVersion", "params": ["token:wrong"]},
        {"methodName": "system.multicall", "params": [[]]},
    ]]});
    let (code, v) = rpc.handle(body.to_string().as_bytes());
    assert_eq!(code.as_u16(), 200);
    let r = v["result"].as_array().unwrap();
    assert_eq!(r[0][0]["numActive"], "0");
    assert_eq!(r[1]["message"], "Unauthorized");
    assert!(r[2]["message"].as_str().unwrap().contains("Recursive"));

    // A JSON-RPC batch works too.
    let body = json!([
        {"jsonrpc": "2.0", "id": 1, "method": "aria2.getVersion", "params": [t]},
        {"jsonrpc": "2.0", "id": 2, "method": "aria2.getSessionInfo", "params": [t]},
    ]);
    let (_, v) = rpc.handle(body.to_string().as_bytes());
    assert_eq!(v.as_array().unwrap().len(), 2);
    assert_eq!(v[1]["id"], 2);
}

async fn http(addr: std::net::SocketAddr, raw: String) -> (u16, String) {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let code = out[9..12].parse().unwrap();
    let body = out.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (code, body)
}

fn post(host: &str, body: &str) -> String {
    format!(
        "POST /jsonrpc HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn over_http_with_rebinding_and_size_guards() {
    let server = Server::start(
        Arc::new(Fake::default()),
        "127.0.0.1:0".parse().unwrap(),
        SECRET.into(),
    )
    .await
    .unwrap();
    let addr = server.addr;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "aria2.getVersion",
                      "params": [format!("token:{SECRET}")]})
    .to_string();
    let (code, out) = http(addr, post(&format!("127.0.0.1:{}", addr.port()), &body)).await;
    assert_eq!(code, 200, "{out}");
    assert!(out.contains("1.37.0"));
    assert_eq!(http(addr, post("localhost:6800", &body)).await.0, 200);
    assert_eq!(http(addr, post("[::1]:6800", &body)).await.0, 200);
    // A web page that pointed its own name at this computer is refused.
    let (code, why) = http(addr, post("evil.example:6800", &body)).await;
    assert_eq!(code, 403);
    assert!(why.contains("127.0.0.1"));

    // Preflight for web front ends, wrong path, wrong method, too big.
    let pre = "OPTIONS /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: http://ariang.example\r\nConnection: close\r\n\r\n".to_string();
    assert_eq!(http(addr, pre).await.0, 204);
    let wrong = "GET /nope HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n".to_string();
    assert_eq!(http(addr, wrong).await.0, 404);
    // The remote page for phones, locked down: no outside resources.
    let page = "GET / HTTP/1.1\r\nHost: 192.168.1.24:6800\r\nConnection: close\r\n\r\n".to_string();
    let (code, html) = http(addr, page).await;
    assert_eq!(code, 200);
    assert!(html.contains("Fuselane remote") && html.contains("jsonrpc"));
    assert!(
        !html.contains("https://cdn"),
        "nothing loaded from elsewhere"
    );
    let get = "GET /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n".to_string();
    assert_eq!(http(addr, get).await.0, 405);
    let big = format!(
        "POST /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        2 * 1024 * 1024
    );
    let (code, why) = http(addr, big).await;
    assert_eq!(code, 413);
    assert!(why.contains("1 MB"));

    // A short secret is refused up front.
    assert!(
        Server::start(
            Arc::new(Fake::default()),
            "127.0.0.1:0".parse().unwrap(),
            "short".into()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn stopping_ends_connections_kept_open() {
    let server = Server::start(
        Arc::new(Fake::default()),
        "127.0.0.1:0".parse().unwrap(),
        SECRET.into(),
    )
    .await
    .unwrap();
    let addr = server.addr;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "aria2.getVersion",
                      "params": [format!("token:{SECRET}")]})
    .to_string();
    let req = format!(
        "POST /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 4096];
    let n = s.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/1.1 200"));

    server.stop().await;
    // The same connection gets nothing more, and the port is free again.
    let _ = s.write_all(req.as_bytes()).await;
    let n = tokio::time::timeout(std::time::Duration::from_secs(2), s.read(&mut buf))
        .await
        .unwrap()
        .unwrap_or(0);
    assert_eq!(n, 0, "closed");
    tokio::net::TcpListener::bind(addr).await.unwrap();
}

/// A client frame: masked, as browsers send them.
fn ws_frame(opcode: u8, data: &[u8]) -> Vec<u8> {
    let mut f = vec![0x80 | opcode];
    let mask = [0x12u8, 0x34, 0x56, 0x78];
    match data.len() {
        n if n < 126 => f.push(0x80 | n as u8),
        n => {
            f.push(0x80 | 126);
            f.extend_from_slice(&(n as u16).to_be_bytes());
        }
    }
    f.extend_from_slice(&mask);
    f.extend(data.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    f
}

async fn ws_read(s: &mut tokio::net::TcpStream) -> (u8, Vec<u8>) {
    let mut h = [0u8; 2];
    s.read_exact(&mut h).await.unwrap();
    let mut len = usize::from(h[1] & 0x7f);
    if len == 126 {
        let mut b = [0u8; 2];
        s.read_exact(&mut b).await.unwrap();
        len = usize::from(u16::from_be_bytes(b));
    }
    let mut data = vec![0u8; len];
    s.read_exact(&mut data).await.unwrap();
    (h[0] & 0x0f, data)
}

/// Opens a WebSocket to `/jsonrpc` the way a browser does; returns the socket
/// and the handshake reply (lowercased).
async fn ws_open(addr: std::net::SocketAddr) -> (tokio::net::TcpStream, String) {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(
        b"GET /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1:6800\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: http://ariang.example\r\n\r\n",
    )
    .await
    .unwrap();
    // Read the handshake reply up to its blank line.
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut b = [0u8; 1];
        s.read_exact(&mut b).await.unwrap();
        head.push(b[0]);
    }
    (s, String::from_utf8(head).unwrap().to_ascii_lowercase())
}

/// One JSON text message from the server, within two seconds.
async fn ws_json(s: &mut tokio::net::TcpStream) -> Value {
    let (op, data) = tokio::time::timeout(std::time::Duration::from_secs(2), ws_read(s))
        .await
        .expect("a message within 2 s");
    assert_eq!(op, 0x1, "a text message");
    serde_json::from_slice(&data).unwrap()
}

#[tokio::test]
async fn websocket_like_ariang_and_closed_when_stopped() {
    let server = Server::start(
        Arc::new(Fake::default()),
        "127.0.0.1:0".parse().unwrap(),
        SECRET.into(),
    )
    .await
    .unwrap();
    let (mut s, head) = ws_open(server.addr).await;
    assert!(head.starts_with("http/1.1 101"), "{head}");
    assert!(head.contains("sec-websocket-accept: s3pplmbitxaq9kygzzhzrbk+xoo="));

    let call = json!({"jsonrpc": "2.0", "id": "a1", "method": "aria2.getVersion",
                      "params": [format!("token:{SECRET}")]})
    .to_string();
    s.write_all(&ws_frame(0x1, call.as_bytes())).await.unwrap();
    let (op, data) = ws_read(&mut s).await;
    assert_eq!(op, 0x1);
    let v: Value = serde_json::from_slice(&data).unwrap();
    assert_eq!(
        (v["id"].as_str(), v["result"]["version"].as_str()),
        (Some("a1"), Some("1.37.0"))
    );

    // Pings get pongs.
    s.write_all(&ws_frame(0x9, b"hi")).await.unwrap();
    assert_eq!(ws_read(&mut s).await, (0xA, b"hi".to_vec()));

    // Stopping (as after a new secret) closes the socket: "going away".
    server.stop().await;
    let (op, data) = tokio::time::timeout(std::time::Duration::from_secs(2), ws_read(&mut s))
        .await
        .unwrap();
    assert_eq!((op, data), (0x8, 1001u16.to_be_bytes().to_vec()));
}

#[tokio::test]
async fn websockets_that_showed_the_secret_get_notifications() {
    let server = Server::start(
        Arc::new(Fake::default()),
        "127.0.0.1:0".parse().unwrap(),
        SECRET.into(),
    )
    .await
    .unwrap();
    let call = |id: &str, token: &str| {
        json!({"jsonrpc": "2.0", "id": id, "method": "aria2.getVersion",
               "params": [format!("token:{token}")]})
        .to_string()
    };
    let (mut shown, _) = ws_open(server.addr).await;
    let (mut stranger, _) = ws_open(server.addr).await;

    // Before showing the secret, nothing is pushed: the reply is the next
    // message, and the notification sent before it never arrives late.
    server.notify(10, Event::Start);
    shown
        .write_all(&ws_frame(0x1, call("auth", SECRET).as_bytes()))
        .await
        .unwrap();
    assert_eq!(ws_json(&mut shown).await["id"], "auth");

    // A wrong secret doesn't count (its refusal waits 250 ms).
    stranger
        .write_all(&ws_frame(0x1, call("guess", "nope-nope-nope").as_bytes()))
        .await
        .unwrap();
    assert_eq!(
        ws_json(&mut stranger).await["error"]["message"],
        "Unauthorized"
    );

    // Each change reaches the socket that showed the secret, as aria2 sends it.
    for (id, event, method) in [
        (10, Event::Start, "aria2.onDownloadStart"),
        (10, Event::Pause, "aria2.onDownloadPause"),
        (11, Event::Stop, "aria2.onDownloadStop"),
        (12, Event::Complete, "aria2.onDownloadComplete"),
        (13, Event::Error, "aria2.onDownloadError"),
    ] {
        server.notify(id, event);
        let v = ws_json(&mut shown).await;
        assert_eq!(
            v,
            json!({"jsonrpc": "2.0", "method": method, "params": [{"gid": gid(id)}]})
        );
        assert!(v.get("id").is_none(), "a notification has no id");
    }

    // The other socket got none of them: a ping's pong is its next message.
    stranger.write_all(&ws_frame(0x9, b"hi")).await.unwrap();
    assert_eq!(ws_read(&mut stranger).await, (0xA, b"hi".to_vec()));

    // Calls still work alongside; a notifier handle reaches sockets too, and
    // a multicall with the secret authenticates the socket as well.
    let (mut multi, _) = ws_open(server.addr).await;
    let body = json!({"jsonrpc": "2.0", "id": "m", "method": "system.multicall", "params": [[
        {"methodName": "aria2.getGlobalStat", "params": [format!("token:{SECRET}")]},
    ]]});
    multi
        .write_all(&ws_frame(0x1, body.to_string().as_bytes()))
        .await
        .unwrap();
    assert_eq!(ws_json(&mut multi).await["id"], "m");
    let notifier = server.notifier();
    notifier.notify(14, Event::Complete);
    assert_eq!(
        ws_json(&mut multi).await["method"],
        "aria2.onDownloadComplete"
    );
    assert_eq!(ws_json(&mut shown).await["params"][0]["gid"], gid(14));

    // After stopping, the notifier goes quiet without failing.
    server.stop().await;
    notifier.notify(15, Event::Start);
}
