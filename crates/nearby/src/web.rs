//! The phone page (B8.12): for a phone without an app, a small page served
//! over plain HTTP on the local network (phones warn about self-signed
//! certificates). The random token in the link is the permission: only someone
//! who saw the QR code can open it. The phone sends files to this computer and
//! saves files offered to it. Nothing here is reachable without the token.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::{Frame, Incoming};
use hyper::{Request, Response, StatusCode};
use tokio::io::AsyncWriteExt;

/// What the app hears from the page.
pub trait PageHost: Send + Sync + 'static {
    /// A file from the phone started arriving.
    fn receiving(&self, id: &str, name: &str, size: u64);
    fn progress(&self, id: &str, written: u64);
    /// It arrived (saved at `path`) or didn't (`Err` says why).
    fn received(&self, id: &str, result: Result<PathBuf, String>);
}

/// A file offered to the phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
}

struct Shared<H> {
    token: String,
    inbox: PathBuf,
    computer: String,
    words: Mutex<Vec<String>>,
    offers: Mutex<Vec<Offer>>,
    host: Arc<H>,
}

/// A running phone page.
pub struct PhonePage {
    pub addr: SocketAddr,
    pub token: String,
    offers: Arc<dyn Fn(Vec<Offer>) + Send + Sync>,
    words: Arc<dyn Fn(Vec<String>) + Send + Sync>,
    task: tokio::task::JoinHandle<()>,
}

impl std::fmt::Debug for PhonePage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhonePage")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl Drop for PhonePage {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn random_token() -> String {
    let mut b = [0u8; 12];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A free name in `dir`, never overwriting.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if first.symlink_metadata().is_err() {
        return first;
    }
    (2..100_000)
        .map(|n| dir.join(fuselane_storage::names::numbered(name, n)))
        .find(|p| p.symlink_metadata().is_err())
        .unwrap_or(first)
}

type Body = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

fn plain(code: StatusCode, msg: &str) -> Response<Body> {
    let mut r = Response::new(
        Full::new(Bytes::from(msg.to_string()))
            .map_err(|n| match n {})
            .boxed(),
    );
    *r.status_mut() = code;
    r.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    r
}

fn with_type(body: Vec<u8>, ty: &'static str) -> Response<Body> {
    let mut r = Response::new(Full::new(Bytes::from(body)).map_err(|n| match n {}).boxed());
    let h = r.headers_mut();
    h.insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static(ty),
    );
    h.insert(
        hyper::header::CACHE_CONTROL,
        hyper::header::HeaderValue::from_static("no-store"),
    );
    h.insert(
        hyper::header::HeaderName::from_static("referrer-policy"),
        hyper::header::HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        hyper::header::HeaderName::from_static("x-content-type-options"),
        hyper::header::HeaderValue::from_static("nosniff"),
    );
    r
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn query(req: &Request<Incoming>) -> HashMap<String, String> {
    url::form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
        .into_owned()
        .collect()
}

impl<H: PageHost> Shared<H> {
    async fn handle(self: Arc<Self>, req: Request<Incoming>) -> Response<Body> {
        let path = req.uri().path().to_string();
        let rest = match path.strip_prefix(&format!("/p/{}", self.token)) {
            Some(r) => r.to_string(),
            None => {
                return plain(
                    StatusCode::NOT_FOUND,
                    "This page has ended. Scan the code on the computer again.",
                );
            }
        };
        match (req.method().as_str(), rest.as_str()) {
            ("GET", "" | "/") => with_type(self.page().into_bytes(), "text/html; charset=utf-8"),
            ("GET", "/files") => {
                let list: Vec<serde_json::Value> = self
                    .offers
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .iter()
                    .map(|o| serde_json::json!({"id": o.id, "name": o.name, "size": o.size}))
                    .collect();
                with_type(
                    serde_json::to_vec(&list).unwrap_or_default(),
                    "application/json",
                )
            }
            ("GET", r) if r.starts_with("/file/") => self.download(&r["/file/".len()..]).await,
            ("POST", "/upload") => self.upload(req).await,
            _ => plain(StatusCode::NOT_FOUND, "Not found"),
        }
    }

    async fn download(&self, id: &str) -> Response<Body> {
        let offer = self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|o| o.id == id)
            .cloned();
        let Some(o) = offer else {
            return plain(StatusCode::NOT_FOUND, "That file isn't offered anymore.");
        };
        let Ok(file) = tokio::fs::File::open(&o.path).await else {
            return plain(
                StatusCode::GONE,
                "That file was moved or deleted on the computer.",
            );
        };
        // Streamed in 256 KB pieces; the length is the size it was offered with.
        let mut r = Response::new(
            FileOut {
                file,
                buf: Vec::new(),
            }
            .boxed(),
        );
        let h = r.headers_mut();
        h.insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/octet-stream"),
        );
        if let Ok(v) = hyper::header::HeaderValue::from_str(&o.size.to_string()) {
            h.insert(hyper::header::CONTENT_LENGTH, v);
        }
        let ascii: String = o
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_graphic() && c != '"' && c != '\\' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let encoded: String = url::form_urlencoded::byte_serialize(o.name.as_bytes()).collect();
        if let Ok(v) = hyper::header::HeaderValue::from_str(&format!(
            "attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
        )) {
            h.insert(hyper::header::CONTENT_DISPOSITION, v);
        }
        r
    }

    async fn upload(&self, req: Request<Incoming>) -> Response<Body> {
        let q = query(&req);
        let name = q.get("name").map(String::as_str).unwrap_or("");
        let last = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
        let clean = fuselane_storage::names::sanitize(last);
        if last.is_empty() || last.starts_with('.') || clean.is_empty() {
            return plain(StatusCode::BAD_REQUEST, "That file name can't be used.");
        }
        let Some(size) = req
            .headers()
            .get(hyper::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        else {
            return plain(StatusCode::LENGTH_REQUIRED, "The file's size is missing.");
        };
        let id = random_token();
        self.host.receiving(&id, &clean, size);
        let part = self
            .inbox
            .join(format!(".{clean}.{}.fuselane-part", &id[..8]));
        let result = self.receive(req, &part, size, &id).await;
        match result {
            Ok(()) => {
                let target = free_name(&self.inbox, &clean);
                match std::fs::rename(&part, &target) {
                    Ok(()) => {
                        self.host.received(&id, Ok(target));
                        plain(StatusCode::OK, "Saved")
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&part);
                        self.host
                            .received(&id, Err(format!("couldn't save it ({e})")));
                        plain(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "The computer couldn't save it.",
                        )
                    }
                }
            }
            Err(why) => {
                let _ = std::fs::remove_file(&part);
                self.host.received(&id, Err(why.clone()));
                plain(StatusCode::BAD_REQUEST, &why)
            }
        }
    }

    async fn receive(
        &self,
        req: Request<Incoming>,
        part: &Path,
        size: u64,
        id: &str,
    ) -> Result<(), String> {
        let mut out = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(part)
            .await
            .map_err(|e| format!("couldn't write it ({e})"))?;
        let mut body = req.into_body();
        let mut written = 0u64;
        let mut last = 0u64;
        while let Some(frame) = tokio::time::timeout(Duration::from_secs(60), body.frame())
            .await
            .map_err(|_| "the phone stopped sending".to_string())?
        {
            let frame = frame.map_err(|_| "the connection dropped".to_string())?;
            let Ok(data) = frame.into_data() else {
                continue;
            };
            written += data.len() as u64;
            if written > size {
                return Err("it was bigger than it said".into());
            }
            out.write_all(&data)
                .await
                .map_err(|e| format!("couldn't write it ({e})"))?;
            if written - last >= 256 * 1024 {
                last = written;
                self.host.progress(id, written);
            }
        }
        if written != size {
            return Err("it didn't arrive in full".into());
        }
        out.sync_all().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    fn page(&self) -> String {
        PAGE.replace("{{computer}}", &escape(&self.computer))
            .replace(
                "{{words}}",
                &self
                    .words
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .iter()
                    .map(|w| format!("<li>{}</li>", escape(w)))
                    .collect::<String>(),
            )
            .replace("{{base}}", &format!("/p/{}", self.token))
    }
}

/// A file sent to the phone in 256 KB pieces.
struct FileOut {
    file: tokio::fs::File,
    buf: Vec<u8>,
}

impl hyper::body::Body for FileOut {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
        use std::task::Poll;
        let me = &mut *self;
        me.buf.resize(256 * 1024, 0);
        let mut rb = tokio::io::ReadBuf::new(&mut me.buf);
        match tokio::io::AsyncRead::poll_read(std::pin::Pin::new(&mut me.file), cx, &mut rb) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Ready(Ok(())) if rb.filled().is_empty() => Poll::Ready(None),
            Poll::Ready(Ok(())) => {
                Poll::Ready(Some(Ok(Frame::data(Bytes::copy_from_slice(rb.filled())))))
            }
        }
    }
}

impl PhonePage {
    /// Starts the page on every IPv4 address, on a free port.
    pub async fn start<H: PageHost>(
        host: Arc<H>,
        inbox: PathBuf,
        computer: String,
        words: Vec<String>,
    ) -> std::io::Result<PhonePage> {
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", 0)).await?;
        let addr = listener.local_addr()?;
        let token = random_token();
        let shared = Arc::new(Shared {
            token: token.clone(),
            inbox,
            computer,
            words: Mutex::new(words),
            offers: Mutex::new(Vec::new()),
            host,
        });
        let word_setter = {
            let shared = shared.clone();
            Arc::new(move |w: Vec<String>| {
                *shared
                    .words
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = w;
            }) as Arc<dyn Fn(Vec<String>) + Send + Sync>
        };
        let setter = {
            let shared = shared.clone();
            Arc::new(move |o: Vec<Offer>| {
                *shared
                    .offers
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = o;
            }) as Arc<dyn Fn(Vec<Offer>) + Send + Sync>
        };
        let task = tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let shared = shared.clone();
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req| {
                        let shared = shared.clone();
                        async move { Ok::<_, std::convert::Infallible>(shared.handle(req).await) }
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(tcp), svc)
                        .await;
                });
            }
        });
        Ok(PhonePage {
            addr,
            token,
            offers: setter,
            words: word_setter,
            task,
        })
    }

    /// The link for `ip` (this computer's address on the phone's network).
    pub fn url(&self, ip: std::net::Ipv4Addr) -> String {
        format!("http://{ip}:{}/p/{}", self.addr.port(), self.token)
    }

    /// The check words the page shows (they depend on its token, known after start).
    pub fn set_words(&self, words: Vec<String>) {
        (self.words)(words);
    }

    /// Replaces the files offered to the phone.
    pub fn offer(&self, offers: Vec<Offer>) {
        (self.offers)(offers);
    }
}

/// The page itself: no external resources, works offline, light and dark.
const PAGE: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer">
<title>Send to {{computer}}</title>
<style>
:root{--bg:#f8f9fb;--card:#fff;--ink:#1b1f27;--body:#454b57;--mute:#6a7180;--line:#e2e5eb;--fuse:#c4501a;--fuse-ink:#fff;color-scheme:light dark}
@media (prefers-color-scheme:dark){:root{--bg:#0d0f13;--card:#15181e;--ink:#eef0f4;--body:#c9cdd5;--mute:#9aa2b1;--line:#262a33;--fuse:#f2894a;--fuse-ink:#0d0f13}}
*{box-sizing:border-box}[hidden]{display:none!important}body{margin:0;background:var(--bg);color:var(--body);font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif}
main{max-width:520px;margin:0 auto;padding:20px 16px 32px;display:grid;gap:18px}
h1{margin:0 0 4px;color:var(--ink);font-size:22px;line-height:1.2}p{margin:0}.mute{color:var(--mute);font-size:13px}
.card{display:grid;gap:12px;padding:14px;border:1px solid var(--line);border-radius:14px;background:var(--card)}
.words{display:grid;grid-template-columns:repeat(4,1fr);gap:6px;margin:0;padding:0;list-style:none}
.words li{padding:8px 0;text-align:center;border:1px solid var(--line);border-radius:8px;font:500 13px ui-monospace,monospace;color:var(--ink)}
.big{display:flex;align-items:center;justify-content:center;gap:8px;min-height:50px;width:100%;border:0;border-radius:10px;background:var(--fuse);color:var(--fuse-ink);font:600 16px system-ui,sans-serif}
.item{display:flex;align-items:center;gap:12px;justify-content:space-between}.item+.item{padding-top:12px;border-top:1px solid var(--line)}
.name{color:var(--ink);overflow-wrap:anywhere}a.save{flex:none;padding:7px 12px;border:1px solid var(--line);border-radius:8px;color:var(--ink);text-decoration:none}
progress{width:100%;height:6px;accent-color:var(--fuse)}
</style>
</head>
<body>
<main>
<div><p class="mute">Connected to</p><h1>{{computer}}</h1><p>This page comes straight from the computer over your Wi-Fi. Nothing to install.</p></div>
<div class="card"><p>The computer shows these words too</p><ol class="words">{{words}}</ol></div>
<label class="big" for="pick">Send photos or files to the computer</label>
<input id="pick" type="file" multiple hidden>
<div id="status" class="card" hidden aria-live="polite"></div>
<section class="card"><p class="mute">Waiting for you</p><div id="files"><p class="mute">Nothing yet. Files the computer offers show up here.</p></div></section>
<p class="mute">Works while Fuselane is open on the computer. The link stops working when sharing is turned off there.</p>
</main>
<script>
const base = "{{base}}"
const status = document.getElementById('status')
function size(n){const u=['B','KB','MB','GB'];let i=0;while(n>=1024&&i<3){n/=1024;i++}return (i?n.toFixed(1):n)+' '+u[i]}
document.getElementById('pick').addEventListener('change', async (e) => {
  const files = [...e.target.files]
  status.hidden = false
  for (const f of files) {
    status.innerHTML = '<p>Sending <b></b></p><progress max="1" value="0"></progress>'
    status.querySelector('b').textContent = f.name
    const bar = status.querySelector('progress')
    const ok = await new Promise((done) => {
      const x = new XMLHttpRequest()
      x.open('POST', base + '/upload?name=' + encodeURIComponent(f.name))
      x.upload.onprogress = (p) => { if (p.lengthComputable) bar.value = p.loaded / p.total }
      x.onload = () => done(x.status === 200 ? '' : x.responseText || 'It didn’t arrive.')
      x.onerror = () => done('The computer can’t be reached. Is Fuselane still open?')
      x.send(f)
    })
    if (ok) { status.textContent = ok; return }
  }
  status.textContent = files.length === 1 ? 'Sent. It’s in the computer’s downloads folder.' : files.length + ' files sent.'
  e.target.value = ''
})
async function refresh(){
  try {
    const r = await fetch(base + '/files', {cache:'no-store'})
    if (!r.ok) return
    const list = await r.json()
    const box = document.getElementById('files')
    if (!list.length) return
    box.replaceChildren(...list.map((f) => {
      const row = document.createElement('div'); row.className = 'item'
      const t = document.createElement('div')
      const n = document.createElement('div'); n.className = 'name'; n.textContent = f.name
      const s = document.createElement('div'); s.className = 'mute'; s.textContent = size(f.size)
      t.append(n, s)
      const a = document.createElement('a'); a.className = 'save'; a.textContent = 'Save'
      a.href = base + '/file/' + encodeURIComponent(f.id); a.download = f.name
      row.append(t, a); return row
    }))
  } catch {}
}
refresh(); setInterval(refresh, 3000)
</script>
</body>
</html>
"#;
