//! `fuselane-desktop --self-test`: a headless check of the packaged app, run by the
//! release workflow on every OS before anything is published (L-75). It never
//! opens a window, so it works on CI runners without a display.

use std::sync::Arc;
use std::time::Duration;

use fuselane_engine_http::download::{BoxIo, Connect, Network, Source, Tuning, download};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// One check's outcome.
#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn check(name: &'static str, r: Result<String, String>) -> Check {
    match r {
        Ok(detail) => Check {
            name,
            ok: true,
            detail,
        },
        Err(detail) => Check {
            name,
            ok: false,
            detail,
        },
    }
}

/// A tiny ranged HTTP server on loopback serving `body` (enough for the engine).
async fn serve(body: Arc<Vec<u8>>) -> std::io::Result<std::net::SocketAddr> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let body = body.clone();
            tokio::spawn(async move {
                loop {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        match sock.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                        if buf.len() > 16 * 1024 {
                            return;
                        }
                    }
                    let head = String::from_utf8_lossy(&buf).to_ascii_lowercase();
                    let size = body.len() as u64;
                    let (first, last) = head
                        .lines()
                        .find_map(|l| l.strip_prefix("range: bytes="))
                        .and_then(|r| {
                            let (a, b) = r.trim().split_once('-')?;
                            let a: u64 = a.parse().ok()?;
                            let b: u64 = if b.is_empty() {
                                size - 1
                            } else {
                                b.parse().ok()?
                            };
                            Some((a, b.min(size - 1)))
                        })
                        .unwrap_or((0, size - 1));
                    let part = &body[first as usize..=last as usize];
                    let resp = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {first}-{last}/{size}\r\nContent-Length: {}\r\nETag: \"selftest\"\r\nAccept-Ranges: bytes\r\n\r\n",
                        part.len()
                    );
                    if sock.write_all(resp.as_bytes()).await.is_err()
                        || sock.write_all(part).await.is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    Ok(addr)
}

/// A real bonded download over two loopback "networks", checked byte for byte.
async fn loopback_download() -> Result<String, String> {
    let body: Arc<Vec<u8>> = Arc::new(
        (0..2_000_000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect(),
    );
    let addr = serve(body.clone()).await.map_err(|e| e.to_string())?;
    let connect: Connect = Arc::new(|a| {
        Box::pin(async move {
            let s = tokio::net::TcpStream::connect(a).await?;
            Ok(Box::new(s) as BoxIo)
        })
    });
    let nets = vec![
        Network {
            id: 1,
            name: "a".into(),
            connect: connect.clone(),
        },
        Network {
            id: 2,
            name: "b".into(),
            connect,
        },
    ];
    let dir = std::env::temp_dir().join(format!("fuselane-selftest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tuning = Tuning {
        block_size: Some(256 * 1024),
        ..Tuning::default()
    };
    let src = Source {
        addr,
        host: addr.to_string(),
        path: "/selftest.bin".into(),
    };
    let result =
        tokio::time::timeout(Duration::from_secs(30), download(src, nets, &dir, tuning)).await;
    let outcome = match result {
        Err(_) => Err("timed out".to_string()),
        Ok(Err(e)) => Err(format!("{e:?}")),
        Ok(Ok(report)) => {
            let got = std::fs::read(&report.path).map_err(|e| e.to_string())?;
            if got == *body {
                Ok(format!("{} bytes over 2 networks, byte-exact", got.len()))
            } else {
                Err("downloaded bytes differ".into())
            }
        }
    };
    let _ = std::fs::remove_dir_all(&dir);
    outcome
}

pub fn run() -> Vec<Check> {
    let mut out = vec![
        check("version", Ok(env!("CARGO_PKG_VERSION").to_string())),
        check(
            "pinning",
            fuselane_transport::self_test()
                .map(|()| "interface pinning works".into())
                .map_err(|e| e.to_string()),
        ),
        check(
            "tls",
            fuselane_transport::tls_ready().map(|()| "crypto and OS trust store ready".into()),
        ),
        check(
            "networks",
            fuselane_netif::list()
                .map(|l| {
                    format!(
                        "{} interfaces, {} usable",
                        l.len(),
                        l.iter().filter(|i| i.usable()).count()
                    )
                })
                .map_err(|e| e.to_string()),
        ),
    ];
    let dir = std::env::temp_dir().join(format!("fuselane-selftest-db-{}", std::process::id()));
    let store = std::fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())
        .and_then(|()| fuselane_core::Store::open(&dir.join("jobs.db")).map_err(|e| e.to_string()))
        .map(|_| "SQLite opens and migrates".to_string());
    let _ = std::fs::remove_dir_all(&dir);
    out.push(check("store", store));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build();
    out.push(check(
        "download",
        match rt {
            Ok(rt) => rt.block_on(loopback_download()),
            Err(e) => Err(e.to_string()),
        },
    ));
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_check_passes_on_a_dev_machine() {
        let checks = super::run();
        for c in &checks {
            assert!(c.ok, "{}: {}", c.name, c.detail);
        }
        assert!(checks.iter().any(|c| c.name == "download"));
    }
}
