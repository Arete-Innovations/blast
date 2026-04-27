
use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tokio::sync::mpsc;

use crate::cata_log;
use crate::meltdown::{MeltDown, MeltType};
use crate::structs::ws::connection::UserId;
use super::protocol::{ClientMessage, ServerMessage};
use super::registry::{OutboundFrame, Registry, SubscriberHandle};

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
        loop {
            let Some(frame) = rx.recv().await else {
                break;
            };
            if sink.send(Message::Text(frame)).await.is_err() {
                break;
            }
        }
    });

    loop {
        let Some(stream_item) = stream.next().await else {
            break;
        };
        let msg = match stream_item {
            Ok(m) => m,
            Err(e) => {
                cata_log!(Debug, format!("ws stream err: {}", e));
                break;
            }
        };
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Subscribe { topic }) => {
                        if !allow_all(&topic) {
                            log_send(send_frame(&tx, ServerMessage::error(&topic, "forbidden")).await);
                            continue;
                        }
                        registry.subscribe(topic.clone(), handle.clone());
                        subscriptions.insert(topic.clone());
                        log_send(send_frame(&tx, ServerMessage::ack(topic)).await);
                    }
                    Ok(ClientMessage::Unsubscribe { topic }) => {
                        registry.unsubscribe(&topic, subscriber_id);
                        subscriptions.remove(&topic);
                    }
                    Ok(ClientMessage::Ping) => {
                        log_send(send_frame(&tx, ServerMessage::pong()).await);
                    }
                    Err(e) => {
                        cata_log!(Debug, format!("ws frame parse: {}", e));
                        log_send(send_frame(&tx, ServerMessage::error_global("malformed_frame")).await);
                    }
                }
            }
            Message::Ping(_payload) => {}
            Message::Pong(_) => {}
            Message::Binary(_) => {
                log_send(send_frame(&tx, ServerMessage::error_global("binary_not_supported")).await);
            }
            Message::Close(_) => break,
        }
    }

    registry.unsubscribe_all(subscriber_id);
    subscriptions.clear();
    drop(tx);
    match outbound.await {
        Ok(()) => {}
        Err(e) => cata_log!(Warning, format!("ws outbound task panicked: {}", e)),
    }
}

async fn send_frame(
    tx: &mpsc::Sender<OutboundFrame>,
    msg: ServerMessage,
) -> Result<(), MeltDown> {
    let encoded = serde_json::to_string(&msg).map_err(|e| {
        MeltDown::new(MeltType::SerializationFailed, format!("ws frame encode: {}", e))
    })?;
    tx.send(encoded).await.map_err(|e| {
        MeltDown::new(MeltType::Unexpected("ws_send".into()), format!("ws send: {}", e))
    })?;
    Ok(())
}

fn log_send(result: Result<(), MeltDown>) {
    match result {
        Ok(()) => {}
        Err(e) => cata_log!(Warning, format!("ws send_frame failed: {}", e)),
    }
}

fn allow_all(_topic: &str) -> bool {
    true
}
