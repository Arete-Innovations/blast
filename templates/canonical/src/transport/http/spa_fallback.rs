//! SPA fallback for Axum: serves `frontend/dist/index.html` for any path not
//! matched by API or WebSocket routes, enabling vue-router history mode.
//!
//! # Usage
//!
//! ```rust,ignore
//! use catalyst::transport::http::spa_fallback::spa_fallback_service;
//!
//! let app = Router::new()
//!     .nest("/api", api_routes)
//!     .route("/ws", get(ws_handler))
//!     .fallback_service(spa_fallback_service("frontend/dist"));
//! ```
//!
//! Static assets under the dist dir are served by `ServeDir`. Unmatched paths
//! (vue-router deep links like `/users/42`) fall through to `index.html`.
//! Existing routes — `/api/*` and `/ws` — have precedence and are never caught.

use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tower_http::services::{ServeDir, ServeFile};

/// Build the SPA fallback service for a given dist directory.
///
/// `dist_dir` should point at the built frontend artefact directory, e.g.
/// `"frontend/dist"`. Within that directory, `index.html` must exist at the
/// root.
///
/// Returns a `ServeDir` service with a `ServeFile` fallback configured; both
/// are already in `tower-http` which is a required dep.
pub fn spa_fallback_service(
    dist_dir: &str,
) -> ServeDir<ServeFile> {
    let index_html = format!("{}/index.html", dist_dir);
    ServeDir::new(dist_dir).fallback(ServeFile::new(index_html))
}

/// Axum handler that serves `index.html` from `dist_dir` directly.
///
/// Used when you need a named fallback handler (e.g. `Router::fallback`) rather
/// than a full `fallback_service`. Reads the file on each call — suitable for
/// development; production traffic hits the static asset cache upstream.
pub async fn spa_index_handler(dist_dir: &str) -> Response {
    let path = format!("{}/index.html", dist_dir);
    match tokio::fs::read(path).await {
        Ok(contents) => axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(Body::from(contents))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        response::Response,
        routing::get,
        Router,
    };
    use tower::util::ServiceExt;

    /// Write a minimal index.html to a temp dir and return the path.
    fn make_dist_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let index = dir.path().join("index.html");
        std::fs::write(&index, b"<!doctype html><html><body>SPA</body></html>")
            .expect("write index.html");
        // Write a known static asset so we can verify asset serving.
        let js = dir.path().join("app.js");
        std::fs::write(js, b"console.log('app')").expect("write app.js");
        dir
    }

    async fn send(app: Router, uri: &str) -> Response {
        app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    // -----------------------------------------------------------------------
    // Fallback: unmatched paths → index.html
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn unmatched_path_serves_index_html() {
        let dir = make_dist_dir();
        let dist = dir.path().to_str().unwrap().to_owned();

        let app = Router::new()
            .fallback_service(spa_fallback_service(&dist));

        let resp = send(app, "/users/42").await;
        assert_eq!(resp.status(), StatusCode::OK);

        let ct = resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("text/html"), "expected text/html, got {ct}");
    }

    // -----------------------------------------------------------------------
    // Static asset: known file is served, not the fallback
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn known_static_asset_served_directly() {
        let dir = make_dist_dir();
        let dist = dir.path().to_str().unwrap().to_owned();

        let app = Router::new()
            .fallback_service(spa_fallback_service(&dist));

        let resp = send(app, "/app.js").await;
        assert_eq!(resp.status(), StatusCode::OK);

        let ct = resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("javascript") || ct.contains("application"), "expected JS content-type, got {ct}");
    }

    // -----------------------------------------------------------------------
    // API routes take precedence over the fallback
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn api_routes_not_caught_by_fallback() {
        let dir = make_dist_dir();
        let dist = dir.path().to_str().unwrap().to_owned();

        let api = Router::new()
            .route("/ping", get(|| async { (StatusCode::OK, "pong") }));

        let app = Router::new()
            .nest("/api", api)
            .fallback_service(spa_fallback_service(&dist));

        // API route hits the handler, not the SPA fallback.
        let resp = send(app.clone(), "/api/ping").await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body_bytes[..], b"pong");
    }

    // -----------------------------------------------------------------------
    // Root path also serves index.html
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn root_path_serves_index_html() {
        let dir = make_dist_dir();
        let dist = dir.path().to_str().unwrap().to_owned();

        let app = Router::new()
            .fallback_service(spa_fallback_service(&dist));

        let resp = send(app, "/").await;
        // ServeDir appends index.html on "/" automatically.
        assert!(
            resp.status() == StatusCode::OK || resp.status() == StatusCode::PERMANENT_REDIRECT,
            "unexpected status {}",
            resp.status()
        );
    }
}
