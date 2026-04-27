
use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tokio::sync::mpsc;

use super::protocol::{ClientMessage, ServerMessage};
use super::registry::{OutboundFrame, Registry, SubscriberHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub i64);

pub const OUTBOUND_QUEUE_DEPTH: usize = 64;

pub fn handle_ws_upgrade(
    ws: WebSocketUpgrade,
    user_id: UserId,
    registry: Arc<Registry>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, registry))
}

pub async fn handle_socket(socket: WebSocket, _user_id: UserId, registry: Arc<Registry>) {
    let (mut sink, mut stream) = socket.split();

    let (tx, mut rx) = mpsc::channel::<OutboundFrame>(OUTBOUND_QUEUE_DEPTH);
    let subscriber_id = registry.next_id();
    let handle = SubscriberHandle {
        id: subscriber_id,
        sender: tx.clone(),
    };

    let mut subscriptions: HashSet<String> = HashSet::new();

    let outbound = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if sink.send(Message::Text(frame)).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = stream.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Subscribe { topic }) => {
                        if !allow_all(&topic) {
                            let _ = send_frame(&tx, ServerMessage::error(&topic, "forbidden"))
                                .await;
                            continue;
                        }
                        registry.subscribe(topic.clone(), handle.clone());
                        subscriptions.insert(topic.clone());
                        let _ = send_frame(&tx, ServerMessage::ack(topic)).await;
                    }
                    Ok(ClientMessage::Unsubscribe { topic }) => {
                        registry.unsubscribe(&topic, subscriber_id);
                        subscriptions.remove(&topic);
                    }
                    Ok(ClientMessage::Ping) => {
                        let _ = send_frame(&tx, ServerMessage::pong()).await;
                    }
                    Err(_) => {
                        let _ =
                            send_frame(&tx, ServerMessage::error_global("malformed_frame")).await;
                    }
                }
            }
            Message::Ping(payload) => {
                let _ = payload;
            }
            Message::Pong(_) => {}
            Message::Binary(_) => {
                let _ = send_frame(&tx, ServerMessage::error_global("binary_not_supported")).await;
            }
            Message::Close(_) => break,
        }
    }

    registry.unsubscribe_all(subscriber_id);
    subscriptions.clear();
    drop(tx);
    let _ = outbound.await;
}

async fn send_frame(
    tx: &mpsc::Sender<OutboundFrame>,
    msg: ServerMessage,
) -> Result<(), ()> {
    let encoded = serde_json::to_string(&msg).map_err(|_| ())?;
    tx.send(encoded).await.map_err(|_| ())
}

fn allow_all(_topic: &str) -> bool {
    true
}
