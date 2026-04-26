# SPEC_LOGGING

Structured logging for Catalyst apps. Two layers: a thin macro + `#[track_caller]` wrapper fns over `tracing`, and a `tower_http::TraceLayer` for per-request spans. See `src/logger.rs` for the authoritative implementation.

## `cata_log!` Macro

The primary logging interface. Five levels:

```rust
cata_log!(Debug,   "schema_parser: parsed 12 tables");
cata_log!(Info,    "bootstrap: database pool initialized");
cata_log!(Warning, format!("missing .env: {}", e));
cata_log!(Error,   format!("flow failed: {}", err));
cata_log!(Trace,   "relay: tick");
```

Each arm dispatches to a corresponding `#[track_caller]` wrapper function. The wrapper function reads `std::panic::Location::caller()` and attaches `src.file` and `src.line` as structured fields on the emitted `tracing` event. Result: structured log output always includes the user's call site, not the logger file.

### `#[track_caller]` wrapper functions

`src/logger.rs` exposes:

```rust
pub fn log_debug(msg: impl AsRef<str>)
pub fn log_info(msg: impl AsRef<str>)
pub fn log_warn(msg: impl AsRef<str>)
pub fn log_error(msg: impl AsRef<str>)
pub fn log_trace(msg: impl AsRef<str>)
```

Each is `#[track_caller]`. They should not be called directly from app code — use `cata_log!` so the macro expansion resolves `caller()` at the actual user call site.

### Structured fields convention

All `cata_log!` events emit two standard structured fields automatically:

| Field | Source | Value |
|-------|--------|-------|
| `src.file` | `Location::caller().file()` | relative source path |
| `src.line` | `Location::caller().line()` | line number |

App code may add extra fields by calling the `tracing` crate directly for cases that need richer context:

```rust
tracing::info!(user_id = ctx.session.user_id, order_id = id, "order created");
```

Use `cata_log!` for plain text messages. Use `tracing::*` macros directly when attaching typed structured fields.

## Tower Trace Layer

The Axum app installs `tower_http::trace::TraceLayer` on every request. This creates a `tracing::Span` per HTTP request containing:

- `http.method`
- `http.uri`
- `http.status_code` (populated on response)
- Latency measurement

Wired in `src/main.rs` via `ServiceBuilder`:

```rust
ServiceBuilder::new()
    .layer(TraceLayer::new_for_http())
    // ...
```

All `cata_log!` calls inside a request's async task are automatically associated with this span, so structured log backends (JSON, OTLP) can correlate them by request.

## Subscriber Init

`src/main.rs` initializes the subscriber at startup:

```rust
tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "catalyst=debug,tower_http=debug,axum::rejection=trace".into()))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

### JSON formatter switch

In production, swap the `fmt::layer()` for JSON output:

```rust
.with(tracing_subscriber::fmt::layer().json())
```

Controlled via `LOG_FORMAT` env var (declared in `storage/blast/state/app.ron`). Blast scaffolds the conditional in `src/main.rs` when `blast new` generates the app. Two formats:

| `LOG_FORMAT` | Formatter | Output |
|-------------|-----------|--------|
| `text` (default) | `fmt::layer()` | Human-readable, colored |
| `json` | `fmt::layer().json()` | Newline-delimited JSON for log aggregators |

Dev default: `text`. Production default: `json`. Blueprint declares the env var with `EnvKind::Enum(&["text", "json"])`.

## Log Level

Controlled via `LOG_LEVEL` env var (standard `tracing_subscriber` `EnvFilter`).

`LOG_LEVEL` is declared in `storage/blast/state/app.ron` as an enum env var with allowed values `["trace","debug","info","warn","error"]` and default `"info"`. At boot, `EnvFilter::try_from_default_env()` reads it. If unset or invalid, the fallback filter (`catalyst=debug,tower_http=debug,axum::rejection=trace`) applies.

Fine-grained per-crate filtering (`RUST_LOG=catalyst=debug,tower_http=warn`) is also supported via the same `EnvFilter` mechanism.

## Structured Fields Convention

For app code beyond plain text:

```rust
// prefer this for important domain events:
tracing::info!(
    user_id = %ctx.session.user_id,
    flow    = "orders::create",
    "order created"
);

// prefer cata_log! for internal / plumbing messages:
cata_log!(Debug, format!("schema_parser: {} tables found", tables.len()));
```

Structured fields are searchable in any JSON-aware log aggregator. Keep field names lowercase with dots for namespacing: `src.file`, `http.method`, `user.id`.

## Anti-Patterns

**Using `println!` or `eprintln!` in app code:**

All output goes through `tracing`. `println!` bypasses the subscriber and won't appear in JSON output, won't carry span context, and won't be filterable. Exception: Blast CLI uses its own `logger` module — that's dev-tooling, not app code.

**`tracing::info!` directly in place of `cata_log!` for plain messages:**

Fine at the `tracing::info!("message")` level, but loses the `src.file` / `src.line` fields that `cata_log!` provides via `#[track_caller]`. Prefer `cata_log!` for plain-text call sites; use `tracing::*` directly only when attaching additional structured fields.

**Logging inside `MeltDown::into_response`:**

`MeltDown`'s `IntoResponse` impl does NOT log. Logging at the call site (flow or route handler) that created the error is preferable — it has more context. Do not re-add logging inside the error type.

## Related Specs

- `SPEC_MELTDOWN.md` — error type; logging policy around error responses
- `SPEC_SESSIONS.md` — session middleware; user_id available in span context
- `catalyst/src/logger.rs` — authoritative implementation
