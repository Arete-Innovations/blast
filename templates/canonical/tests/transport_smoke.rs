use axum::{body::Body, http::Request};
use canonical::{transport::http::router, Ctx};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use tower::ServiceExt;

fn build_pool(url: &str) -> Result<Pool<AsyncPgConnection>, canonical::meltdown::MeltDown> {
    let cfg = AsyncDieselConnectionManager::<AsyncPgConnection>::new(url);
    Pool::builder(cfg)
        .max_size(2)
        .build()
        .map_err(|e| canonical::meltdown::MeltDown::db_connection(format!("test pool build failed: {}", e)))
}

#[tokio::test]
async fn healthz_returns_200() -> Result<(), canonical::meltdown::MeltDown> {
    let url = match std::env::var("DATABASE_URL_TEST") {
        Ok(u) => u,
        Err(_) => return Ok(()),
    };

    let pool = build_pool(&url)?;
    let ctx = Ctx::anonymous(pool);
    let app = router(ctx);

    let request = Request::builder()
        .method("GET")
        .uri("/healthz")
        .body(Body::empty())
        .map_err(|e| canonical::meltdown::MeltDown::bad_request(format!("request build failed: {}", e)))?;

    let response = app.oneshot(request).await.map_err(|e| canonical::meltdown::MeltDown::db_connection(format!("oneshot failed: {}", e)))?;

    assert_eq!(response.status(), axum::http::StatusCode::OK);

    Ok(())
}
