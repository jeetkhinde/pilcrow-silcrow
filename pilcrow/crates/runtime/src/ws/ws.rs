// ./src/ws.rs

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use std::future::Future;

crate::define_route!(WsRoute, "WebSocket", "/ws/chat", "CHAT");

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    Patch {
        target: String,
        data: serde_json::Value,
    },
    Html {
        target: String,
        markup: String,
    },
    Invalidate {
        target: String,
    },
    Navigate {
        path: String,
    },
    Custom {
        event: String,
        data: serde_json::Value,
    },
}

impl WsEvent {
    pub fn patch(data: impl serde::Serialize, target: &str) -> Self {
        let value = crate::serialize_or_null(data, "WsEvent::patch");
        Self::Patch {
            target: target.to_owned(),
            data: value,
        }
    }

    pub fn html(markup: impl Into<String>, target: &str) -> Self {
        Self::Html {
            target: target.to_owned(),
            markup: markup.into(),
        }
    }

    pub fn invalidate(target: &str) -> Self {
        Self::Invalidate {
            target: target.to_owned(),
        }
    }

    pub fn navigate(path: impl Into<String>) -> Self {
        Self::Navigate { path: path.into() }
    }

    pub fn custom(event: impl Into<String>, data: impl serde::Serialize) -> Self {
        let value = crate::serialize_or_null(data, "WsEvent::custom");
        Self::Custom {
            event: event.into(),
            data: value,
        }
    }
}

#[derive(Debug)]
pub enum WsRecvError {
    Deserialize(serde_json::Error),
    Closed,
    NonText,
}

impl std::fmt::Display for WsRecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deserialize(e) => write!(f, "WsRecvError::Deserialize: {e}"),
            Self::Closed => write!(f, "WsRecvError::Closed"),
            Self::NonText => write!(f, "WsRecvError::NonText"),
        }
    }
}

impl std::error::Error for WsRecvError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Deserialize(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct WsStream {
    socket: WebSocket,
}

impl WsStream {
    /// Wrap an Axum WebSocket in a typed Silcrow stream.
    pub fn new(socket: WebSocket) -> Self {
        Self { socket }
    }

    pub async fn send(&mut self, event: WsEvent) -> Result<(), axum::Error> {
        match serde_json::to_string(&event) {
            Ok(json) => self.socket.send(Message::Text(json)).await,
            Err(e) => {
                tracing::warn!("WsStream::send serialization failed: {e}");
                Err(axum::Error::new(e))
            }
        }
    }
    pub async fn recv(&mut self) -> Option<Result<WsEvent, WsRecvError>> {
        loop {
            match self.socket.recv().await {
                None => return None,
                Some(Err(_)) => return None,
                Some(Ok(msg)) => match msg {
                    Message::Text(text) => {
                        return Some(serde_json::from_str(&text).map_err(WsRecvError::Deserialize));
                    }
                    Message::Close(_) => return Some(Err(WsRecvError::Closed)),
                    Message::Ping(_) | Message::Pong(_) => continue,
                    Message::Binary(_) => return Some(Err(WsRecvError::NonText)),
                },
            }
        }
    }

    /// Gracefully close the WebSocket connection.
    pub async fn close(mut self) {
        let _ = self.socket.send(Message::Close(None)).await;
    }
}

pub fn ws<F, Fut>(upgrade: WebSocketUpgrade, handler: F) -> Response
where
    F: FnOnce(WsStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    upgrade
        .on_upgrade(|socket| async move {
            handler(WsStream::new(socket)).await;
        })
        .into_response()
}
