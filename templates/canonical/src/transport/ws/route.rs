use std::sync::Arc;

use axum::{
    extract::{Extension, State, WebSocketUpgrade},
    response::Response,
};

use crate::{
    transport::ws::{connection::handle_socket, registry::Registry},
    Ctx,
};

pub async fn ws_upgrade(State(registry): State<Arc<Registry>>, Extension(ctx): Extension<Ctx>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, ctx, registry))
}
