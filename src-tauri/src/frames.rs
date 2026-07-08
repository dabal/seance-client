//! Loopback WebSocket channel for framebuffer traffic.
//!
//! Pixel data is server-push and high-volume; shipping it as base64 inside
//! JSON Tauri events was v0 scaffolding. This channel binds 127.0.0.1 on a
//! random port with a one-time token in the URL path; the webview connects
//! once at startup and receives every draw operation as a binary frame.
//!
//! All canvas-mutating operations (rects, copyrects, cursor, resize) must go
//! through this one channel: they are order-sensitive, and splitting them
//! across transports would let a copyrect overtake the rect it depends on.
//!
//! Frame layout, little-endian (matches JS DataView defaults):
//!
//! ```text
//! u8  op          0=rect 1=copyrect 2=cursor 3=resize
//! u32 session     session id (always 0 today; Phase 3 tabs multiplex on it)
//! ... per-op payload:
//!   rect:     u16 x,y,w,h            + w*h*4 RGBA bytes
//!   copyrect: u16 x,y,w,h,srcX,srcY
//!   cursor:   u16 hotX,hotY,w,h      + w*h*4 RGBA bytes
//!   resize:   u16 w,h
//! ```

use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::Message;

const OP_RECT: u8 = 0;
const OP_COPYRECT: u8 = 1;
const OP_CURSOR: u8 = 2;
const OP_RESIZE: u8 = 3;

#[derive(Clone)]
pub struct FrameChannel {
    endpoint: String,
    /// Sender for the currently connected webview, if any. A new connection
    /// replaces the old one (happens on dev-server hot reload).
    slot: Arc<Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>>,
}

impl FrameChannel {
    pub async fn start() -> Result<Self, crate::rfb::Error> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let token = format!("{:032x}", rand::random::<u128>());
        let endpoint = format!("ws://127.0.0.1:{port}/{token}");
        let slot: Arc<Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>> = Arc::new(Mutex::new(None));

        let accept_slot = slot.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let expected = format!("/{token}");
                let check = move |req: &Request, resp: Response| {
                    if req.uri().path() == expected {
                        Ok(resp)
                    } else {
                        let mut deny = ErrorResponse::new(None);
                        *deny.status_mut() =
                            tokio_tungstenite::tungstenite::http::StatusCode::FORBIDDEN;
                        Err(deny)
                    }
                };
                let ws = match tokio_tungstenite::accept_hdr_async(stream, check).await {
                    Ok(ws) => ws,
                    Err(_) => continue,
                };
                let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
                *accept_slot.lock().unwrap() = Some(tx);
                let (mut sink, mut source) = ws.split();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::select! {
                            frame = rx.recv() => match frame {
                                Some(f) => {
                                    if sink.send(Message::Binary(f)).await.is_err() {
                                        break;
                                    }
                                }
                                // sender replaced by a newer connection
                                None => break,
                            },
                            msg = source.next() => match msg {
                                // nothing meaningful arrives from the webview;
                                // tungstenite answers pings internally
                                Some(Ok(_)) => {}
                                _ => break,
                            },
                        }
                    }
                });
            }
        });

        Ok(Self { endpoint, slot })
    }

    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    fn send(&self, frame: Vec<u8>) {
        let mut slot = self.slot.lock().unwrap();
        if let Some(tx) = slot.as_ref() {
            if tx.send(frame).is_err() {
                // webview gone; frames are droppable — it requests a full
                // update when it reconnects
                *slot = None;
            }
        }
    }

    fn header(op: u8, session: u32, extra: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(5 + extra);
        v.push(op);
        v.extend_from_slice(&session.to_le_bytes());
        v
    }

    pub fn send_rect(&self, session: u32, x: u16, y: u16, w: u16, h: u16, rgba: &[u8]) {
        let mut f = Self::header(OP_RECT, session, 8 + rgba.len());
        for v in [x, y, w, h] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(rgba);
        self.send(f);
    }

    pub fn send_copyrect(
        &self,
        session: u32,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        src_x: u16,
        src_y: u16,
    ) {
        let mut f = Self::header(OP_COPYRECT, session, 12);
        for v in [x, y, w, h, src_x, src_y] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        self.send(f);
    }

    pub fn send_cursor(&self, session: u32, hot_x: u16, hot_y: u16, w: u16, h: u16, rgba: &[u8]) {
        let mut f = Self::header(OP_CURSOR, session, 8 + rgba.len());
        for v in [hot_x, hot_y, w, h] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(rgba);
        self.send(f);
    }

    pub fn send_resize(&self, session: u32, w: u16, h: u16) {
        let mut f = Self::header(OP_RESIZE, session, 4);
        for v in [w, h] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        self.send(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn rejects_bad_token_and_delivers_framed_rects() {
        let ch = FrameChannel::start().await.unwrap();
        let endpoint = ch.endpoint();

        let (base, _) = endpoint.rsplit_once('/').unwrap();
        let bad = format!("{base}/{:032x}", 0u128);
        assert!(
            tokio_tungstenite::connect_async(&bad).await.is_err(),
            "handshake with a wrong token must be refused"
        );

        let (mut ws, _) = tokio_tungstenite::connect_async(&endpoint).await.unwrap();

        // the sender is registered just after the server-side handshake, which
        // races this client's return from connect_async — frames sent before
        // registration are dropped by design, so send until one arrives
        let frame = loop {
            ch.send_rect(0, 1, 2, 3, 4, &[9u8; 48]);
            match timeout(Duration::from_millis(100), ws.next()).await {
                Ok(Some(Ok(Message::Binary(b)))) => break b,
                Ok(other) => panic!("unexpected websocket message: {other:?}"),
                Err(_) => {} // not registered yet; retry
            }
        };

        assert_eq!(frame[0], OP_RECT);
        assert_eq!(&frame[1..5], &0u32.to_le_bytes());
        // x=1 y=2 w=3 h=4, little-endian u16s
        assert_eq!(&frame[5..13], &[1, 0, 2, 0, 3, 0, 4, 0]);
        assert_eq!(&frame[13..], &[9u8; 48][..]);
    }
}
