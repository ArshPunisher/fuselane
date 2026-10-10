//! WebSocket for the same JSON-RPC (RFC 6455), because AriaNg connects that
//! way by default. Text messages in, replies out, and aria2's
//! `aria2.onDownload*` notifications pushed once the socket has shown the
//! secret. Written by hand: the protocol needs only a handshake hash and a
//! small frame format.

use std::sync::Arc;

use hyper::StatusCode;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast::error::RecvError;

use crate::{Host, MAX_BODY, Rpc};

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The `Sec-WebSocket-Accept` answer for a client's key.
pub fn accept_key(key: &str) -> String {
    use base64::Engine as _;
    use sha1::Digest as _;
    let hash = sha1::Sha1::digest(format!("{}{GUID}", key.trim()).as_bytes());
    base64::engine::general_purpose::STANDARD.encode(hash)
}

/// One frame from the client: (fin, opcode, payload unmasked).
async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<(bool, u8, Vec<u8>)> {
    let mut head = [0u8; 2];
    r.read_exact(&mut head).await?;
    let fin = head[0] & 0x80 != 0;
    let opcode = head[0] & 0x0f;
    let masked = head[1] & 0x80 != 0;
    let mut len = u64::from(head[1] & 0x7f);
    if len == 126 {
        let mut b = [0u8; 2];
        r.read_exact(&mut b).await?;
        len = u64::from(u16::from_be_bytes(b));
    } else if len == 127 {
        let mut b = [0u8; 8];
        r.read_exact(&mut b).await?;
        len = u64::from_be_bytes(b);
    }
    if !masked {
        // Clients must mask (RFC 6455 §5.1).
        return Err(std::io::Error::other("unmasked frame from a client"));
    }
    if len > MAX_BODY as u64 {
        return Err(std::io::Error::other("message too big"));
    }
    let mut mask = [0u8; 4];
    r.read_exact(&mut mask).await?;
    let mut data = vec![0u8; len as usize];
    r.read_exact(&mut data).await?;
    for (i, b) in data.iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Ok((fin, opcode, data))
}

/// [`read_frame`] owning the reader, so one read can stay pending across many
/// waits: dropping a half-read frame would lose its bytes.
async fn read_owned<R: AsyncRead + Unpin>(mut r: R) -> (R, std::io::Result<(bool, u8, Vec<u8>)>) {
    let frame = read_frame(&mut r).await;
    (r, frame)
}

async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    opcode: u8,
    data: &[u8],
) -> std::io::Result<()> {
    let mut head = vec![0x80 | opcode];
    match data.len() {
        n if n < 126 => head.push(n as u8),
        n if n <= 0xffff => {
            head.push(126);
            head.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            head.push(127);
            head.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    w.write_all(&head).await?;
    w.write_all(data).await?;
    w.flush().await
}

/// Serves one WebSocket until the client closes it.
pub async fn serve<H: Host, S: AsyncRead + AsyncWrite + Unpin>(rpc: Arc<Rpc<H>>, io: S) {
    let (reader, mut io) = tokio::io::split(io);
    let mut reading = Box::pin(read_owned(reader));
    let mut message: Vec<u8> = Vec::new();
    let mut stopped = rpc.stopped.clone();
    let mut notices = rpc.notices.subscribe();
    let mut listening = true;
    // Notifications go only to a socket that has shown the secret once.
    let mut authed = false;
    loop {
        if *stopped.borrow() {
            let _ = write_frame(&mut io, 0x8, &1001u16.to_be_bytes()).await;
            return;
        }
        let frame = tokio::select! {
            // Notifications before the next message: one sent before this
            // socket showed the secret is dropped, never delivered late.
            biased;
            // The server stopped (turned off, or a new secret): "going away".
            _ = stopped.changed() => {
                let _ = write_frame(&mut io, 0x8, &1001u16.to_be_bytes()).await;
                return;
            }
            notice = notices.recv(), if listening => {
                match notice {
                    Ok(n) if authed => {
                        if write_frame(&mut io, 0x1, &n.to_json()).await.is_err() {
                            return;
                        }
                    }
                    // Not shown the secret, or fell behind: the front end's
                    // own polling catches up.
                    Ok(_) | Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => listening = false,
                }
                continue;
            }
            (r, frame) = &mut reading => {
                reading.set(read_owned(r));
                frame
            }
        };
        let Ok((fin, opcode, data)) = frame else {
            // Too big or broken: close with "message too big" / "protocol error".
            let _ = write_frame(&mut io, 0x8, &1009u16.to_be_bytes()).await;
            return;
        };
        match opcode {
            0x8 => {
                let _ = write_frame(&mut io, 0x8, &data[..data.len().min(2)]).await;
                return;
            }
            0x9 => {
                if write_frame(&mut io, 0xA, &data).await.is_err() {
                    return;
                }
                continue;
            }
            0xA => continue,
            0x0..=0x2 => {
                message.extend_from_slice(&data);
                if message.len() > MAX_BODY {
                    let _ = write_frame(&mut io, 0x8, &1009u16.to_be_bytes()).await;
                    return;
                }
                if !fin {
                    continue;
                }
            }
            _ => {
                let _ = write_frame(&mut io, 0x8, &1002u16.to_be_bytes()).await;
                return;
            }
        }
        let body = std::mem::take(&mut message);
        let r = rpc.clone();
        let Ok((code, value, shown)) =
            tokio::task::spawn_blocking(move || r.handle_checked(&body)).await
        else {
            return;
        };
        authed |= shown;
        if code == StatusCode::UNAUTHORIZED {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        let out = serde_json::to_vec(&value).unwrap_or_default();
        if write_frame(&mut io, 0x1, &out).await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handshake_answer_matches_the_rfc_example() {
        // RFC 6455 §1.3.
        assert_eq!(
            accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
}
