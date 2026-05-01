use axum::{body::{to_bytes, Body}, extract::rejection::JsonRejection, http::{Request, StatusCode}, routing::post, Json, Router};
use canonical::meltdown::MeltDown;
use serde::Deserialize;
use serde_json::Value;
use tower::ServiceExt;

#[derive(Debug, Deserialize)]
struct Echo {
    #[allow(dead_code)]
    name: String,
}

async fn echo(body: Result<Json<Echo>, JsonRejection>) -> Result<&'static str, MeltDown> {
    let _ = body?;
    Ok("ok")
}

fn build_app() -> Router {
    Router::new().route("/echo", post(echo))
}

fn req(content_type: Option<&str>, body: &'static str) -> Request<Body> {
    let mut b = Request::builder().method("POST").uri("/echo");
    if let Some(ct) = content_type {
        b = b.header("content-type", ct);
    }
    b.body(Body::from(body)).expect("build req")
}

#[tokio::test]
async fn malformed_json_returns_400_meltdown_envelope() {
    let app = build_app();
    let resp = app.oneshot(req(Some("application/json"), "{not json")).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let bytes = to_bytes(resp.into_body(), 8192).await.expect("body bytes");
    let parsed: Value = serde_json::from_slice(&bytes).expect("body is valid JSON envelope");
    let err = parsed.get("error").expect("error key present");
    assert_eq!(err.get("code").and_then(|v| v.as_u64()), Some(400));
    assert_eq!(err.get("type").and_then(|v| v.as_str()), Some("BadRequest"));
    assert_eq!(err.get("message").and_then(|v| v.as_str()), Some("Invalid request body."));
}

#[tokio::test]
async fn missing_content_type_returns_400_meltdown_envelope() {
    let app = build_app();
    let resp = app.oneshot(req(None, "{\"name\":\"x\"}")).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let bytes = to_bytes(resp.into_body(), 8192).await.expect("body bytes");
    let parsed: Value = serde_json::from_slice(&bytes).expect("body is valid JSON envelope");
    let err = parsed.get("error").expect("error key present");
    assert_eq!(err.get("type").and_then(|v| v.as_str()), Some("BadRequest"));
    assert_eq!(err.get("message").and_then(|v| v.as_str()), Some("Invalid request body."));
}

#[tokio::test]
async fn valid_json_succeeds() {
    let app = build_app();
    let resp = app.oneshot(req(Some("application/json"), "{\"name\":\"x\"}")).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
}
