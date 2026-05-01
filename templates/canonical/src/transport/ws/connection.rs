use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tokio::sync::{mpsc, oneshot};

use crate::{
    cata_log,
    meltdown::{MeltDown, MeltType},
    transport::ws::{
        auth,
        protocol::{ClientMessage, ServerMessage},
        registry::{OutboundFrame, Registry, SubscriberHandle},
    },
    Ctx,
};

pub const OUTBOUND_QUEUE_DEPTH: usize = 64;

pub async fn handle_socket(socket: WebSocket, ctx: Ctx, registry: Arc<Registry>) {
    let user_id = match ctx.session_user_id() {
        Some(id) => id,
        None => {
            cata_log!(Warning, "ws: handle_socket reached with anonymous ctx; closing");
            return;
        }
    };

    let (mut sink, mut stream) = socket.split();

    let (tx, mut rx) = mpsc::channel::<OutboundFrame>(OUTBOUND_QUEUE_DEPTH);
    let subscriber_id = registry.next_id();
    let handle = SubscriberHandle { id: subscriber_id, sender: tx.clone() };

    let (close_tx, mut close_rx) = oneshot::channel::<()>();
    match registry.claim_session(user_id, subscriber_id, close_tx) {
        Some(prev) => {
            match prev.close_signal.send(()) {
                Ok(()) => {}
                Err(()) => cata_log!(Debug, format!("ws: prev session for user {} already closed before evict", user_id)),
            }
            registry.unsubscribe_all(prev.subscriber_id);
            cata_log!(Info, format!("ws: evicted prior subscriber {} for user {}", prev.subscriber_id, user_id));
        }
        None => {}
    }

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
        let msg = tokio::select! {
            _ = &mut close_rx => {
                cata_log!(Debug, format!("ws: subscriber {} evicted via close_signal", subscriber_id));
                break;
            }
            maybe_msg = stream.next() => {
                let Some(stream_item) = maybe_msg else { break; };
                match stream_item {
                    Ok(m) => m,
                    Err(e) => {
                        cata_log!(Debug, format!("ws stream err: {}", e));
                        break;
                    }
                }
            }
        };
        match msg {
            Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                Ok(ClientMessage::Subscribe { topic }) => {
                    if !auth::can_subscribe(&ctx, &topic) {
                        if !log_send(send_frame(&tx, ServerMessage::error(&topic, "forbidden"))) {
                            break;
                        }
                        continue;
                    }
                    registry.subscribe(topic.clone(), handle.clone());
                    if !log_send(send_frame(&tx, ServerMessage::ack(topic))) {
                        break;
                    }
                }
                Ok(ClientMessage::Unsubscribe { topic }) => {
                    registry.unsubscribe(&topic, subscriber_id);
                }
                Ok(ClientMessage::Ping) => {
                    if !log_send(send_frame(&tx, ServerMessage::pong())) {
                        break;
                    }
                }
                Err(e) => {
                    cata_log!(Debug, format!("ws frame parse: {}", e));
                    if !log_send(send_frame(&tx, ServerMessage::error_global("malformed_frame"))) {
                        break;
                    }
                }
            },
            Message::Ping(_payload) => {}
            Message::Pong(_) => {}
            Message::Binary(_) => {
                if !log_send(send_frame(&tx, ServerMessage::error_global("binary_not_supported"))) {
                    break;
                }
            }
            Message::Close(_) => break,
        }
    }

    registry.unsubscribe_all(subscriber_id);
    registry.release_session(user_id, subscriber_id);
    drop(tx);
    match outbound.await {
        Ok(()) => {}
        Err(e) => cata_log!(Warning, format!("ws outbound task panicked: {}", e)),
    }
}

fn send_frame(tx: &mpsc::Sender<OutboundFrame>, msg: ServerMessage) -> Result<(), MeltDown> {
    let encoded = serde_json::to_string(&msg).map_err(|e| MeltDown::new(MeltType::SerializationFailed, format!("ws frame encode: {}", e)))?;
    match tx.try_send(encoded) {
        Ok(()) => Ok(()),
        Err(mpsc::error::TrySendError::Full(_)) => Err(MeltDown::new(MeltType::Unexpected("ws_send".into()), "outbound channel full")),
        Err(mpsc::error::TrySendError::Closed(_)) => Err(MeltDown::new(MeltType::Unexpected("ws_send".into()), "outbound channel closed")),
    }
}

fn log_send(result: Result<(), MeltDown>) -> bool {
    match result {
        Ok(()) => true,
        Err(e) => {
            cata_log!(Warning, format!("ws send_frame failed: {}", e));
            false
        }
    }
}
