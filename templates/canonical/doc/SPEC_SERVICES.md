# SPEC_SERVICES

Stateless adapter layer. Catalyst ships **one** implementation per service. No trait abstraction, no driver enum, no plugin hook. If you want a different backend, fork.

Lives in `crate::services`. Layered above `models/` and below `routines/` (see `SPEC_ARCHITECTURE.md`).

## Why One Impl Each

Trait-based service abstractions are how stack drift starts. The moment `Storage` is `Box<dyn StorageBackend>` someone adds an S3 driver, then a GCS driver, then config knobs to pick between them. The matrix of supported configurations explodes; nothing is opinionated anymore.

Catablast targets one VPS-hosted monolith with ~10K users. For that target:

- **Storage:** local disk is fast, free, easy to back up with `tar`.
- **Email:** SMTP is the lowest-common-denominator wire, supported by every provider (SES, Mailgun, Postmark, your own postfix).
- **Rate-limit:** in-memory is correct for one binary. There is no distributed coordinator to sync with.

Each service is a concrete struct. Constructed via `from_env()`. Stashed in app state. Used directly. No indirection.

If your scale needs S3, Sidekiq-style queues, or Redis rate-limit — fork. Replace `services::storage` with whatever you want. The rest of the stack only sees the concrete `Storage` struct, so you only have one consumer site to update.

## Module Layout

```
src/services/
├── mod.rs
├── crypto.rs         (host-only — session token gen + sha256)
├── email.rs          (host-only — lettre SMTP transport)
├── external_http.rs  (host-only — reqwest helpers)
├── rate_limit.rs     (host-only — in-memory token bucket)
├── storage.rs        (host-only — local-disk file storage)
├── time.rs           (host-only — wall-clock helpers)
└── render/           (cross-target — SSR component builders)
    ├── mod.rs
    ├── table.rs / table.module.scss
    ├── form.rs / form.module.scss
    ├── list.rs / list.module.scss
    ├── select.rs / select.module.scss
    ├── detail.rs / detail.module.scss
    └── stat.rs / stat.module.scss
```

Host-only modules are gated `#[cfg(not(target_arch = "wasm32"))]` in `services/mod.rs`. The `render/` family is cross-target and compiles on both host (SSR) and wasm32 (hydrate). Struct/enum data definitions for render builders live in `structs/services/render/` per `STRUCTS:22`.

See `SPEC_LEPTOS.md` "Render service" section for the builder API surface — `TableBuilder`, `FormBuilder`, `ListBuilder`, `SelectBuilder`, `DetailBuilder`, `StatBuilder`.

## Conventions (All Services)

- `Result<T, MeltDown>` everywhere. Map external errors to `MeltType::ExternalServiceError`, `FileOperationFailed`, etc.
- `from_env() -> Result<Self, MeltDown>` for construction. Reads env vars directly. Missing required vars → `MeltType::EnvironmentError`.
- No `Default` impls. No globals. Catalyst doesn't impose where the user stashes the service — the user's app holds them in app state alongside the DB pool.
- No async traits. Plain methods. `email::Email::send` is `async` because lettre's transport is; the rest are sync.
- Logging via `cata_log!`. No `println!`, no `eprintln!`.
- No `anyhow`, no `Box<dyn Error>` in public signatures.

## Storage

Local disk only. Files live under a configurable root directory. All paths are interpreted relative to that root.

### Env

| Var | Default | Required |
|-----|---------|----------|
| `STORAGE_ROOT` | `./storage` | no |

### API

```rust
pub struct Storage {  }

impl Storage {
    pub fn from_env() -> Result<Storage, MeltDown>;

    pub fn put(&self, path: &str, bytes: &[u8]) -> Result<(), MeltDown>;
    pub fn get(&self, path: &str) -> Result<Vec<u8>, MeltDown>;
    pub fn delete(&self, path: &str) -> Result<(), MeltDown>;
    pub fn list(&self, prefix: &str) -> Result<Vec<String>, MeltDown>;
    pub fn exists(&self, path: &str) -> bool;
}
```

### Path Normalization

User-supplied paths are validated before joining with the root.

Rejected with `MeltType::FilePermissionDenied`:
- absolute paths (starts with `/`)
- any segment equal to `..`
- empty path

Allowed:
- forward-slash separated relative paths (`avatars/123.png`, `posts/42/cover.svg`)
- bare filenames (`favicon.ico`)

`put` creates parent directories on demand. `get` returns `MeltType::FileNotFound` if missing. `delete` is idempotent — deleting a missing file is `Ok(())`.

`list(prefix)` walks the root, returns relative paths whose string-form starts with `prefix`. Returned paths use forward-slashes regardless of host OS. Sorted lexicographically.

### Streaming

Out of scope for the v1. `put`/`get` take/return `Vec<u8>`. If you need multi-GB blobs you've outgrown one VPS — fork or wire S3 at the routine layer.

## Email

SMTP via [`lettre`](https://docs.rs/lettre). Async transport with rustls.

### Env

| Var | Default | Required |
|-----|---------|----------|
| `SMTP_HOST` | — | yes |
| `SMTP_PORT` | `587` | no |
| `SMTP_USER` | — | yes |
| `SMTP_PASS` | — | yes |
| `SMTP_FROM` | — | yes (RFC-5322 mailbox) |
| `SMTP_TLS` | `true` | no (`true`/`false`) |

`SMTP_TLS=true` → STARTTLS on the configured port. `false` → plaintext (only sane for localhost dev relays).

### API

```rust
pub struct Email {  }

impl Email {
    pub fn from_env() -> Result<Email, MeltDown>;

    pub async fn send(
        &self,
        to: &str,
        subject: &str,
        body_text: &str,
        body_html: Option<&str>,
    ) -> Result<(), MeltDown>;
}
```

`to` must parse as a single RFC-5322 mailbox (`MeltType::ValidationFailed` otherwise). When `body_html` is `Some`, the message is sent multipart/alternative; the text part is always present.

### Error Mapping

- Bad mailbox / bad subject → `ValidationFailed`
- Transport failure (network, bad TLS handshake, server reject) → `ExternalServiceError` with `mark_transient(true)`
- Auth failure → `ExternalServiceError` with `mark_transient(false)` (a bad password won't fix itself by retrying)

Pair with `Crank` at the routine layer for retries:

```rust
Crank::new(policy)
    .classify(MeltDown::is_transient)
    .run(|| email.send(&to, "Welcome", &text, None))
    .await?;
```

### Templating

Out of scope for `services::email`. Render bodies in routines; pass raw strings to `send`. Catalyst doesn't ship a template engine.

## Rate Limit

Token bucket per key, stored in a `DashMap`. Process-local. Restart resets all buckets.

### Env

None. Limits are passed per-call by the caller.

### API

```rust
pub struct RateLimit {  }

impl RateLimit {
    pub fn new() -> RateLimit;



    pub fn check_and_consume(&self, key: &str, max: u32, window: Duration) -> bool;
}

struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
}
```

### Algorithm

Token bucket with continuous refill:

1. Lookup or insert (with `tokens = max`) the bucket for `key`.
2. Compute elapsed since `last_refill`.
3. Refill rate = `max / window` tokens per second. Add `elapsed * rate` tokens, capped at `max`.
4. Set `last_refill = now` (always, even when no integer tokens accrued yet — fractional remainder is dropped; acceptable for our scale).
5. If `tokens >= 1` → decrement, return `true`. Else return `false`.

`max` and `window` are call-site arguments, not bucket-state. The caller is the source of truth; changing the policy at the call site changes behavior on the next request without resetting buckets. (Side effect: two callers passing different `max` for the same `key` will fight. Don't do that.)

### Why Not Distributed

One binary. One DashMap. No coordination. If you scale to multiple binaries you're out of the supported config; fork to plug in Redis.

### Why Not Persistent

Process restart resets buckets. Acceptable: the worst case is a very brief window where rate-limited callers can re-burst. The simplicity is the feature.

## Singletons

Catalyst doesn't impose a global. The user's app constructs services in `bootstrap.rs` and stashes them in app state:

```rust
pub struct AppState {
    pub db: DbPool,
    pub storage: Arc<Storage>,
    pub email: Arc<Email>,
    pub rate_limit: Arc<RateLimit>,
}

let state = AppState {
    db: database::pool::build()?,
    storage: Arc::new(Storage::from_env()?),
    email: Arc::new(Email::from_env()?),
    rate_limit: Arc::new(RateLimit::new()),
};
```

Routines take `&AppState` and reach for whichever services they need.

## Fork-To-Swap

If you need a different backend:

1. Fork Catalyst.
2. Replace the body of `src/services/<name>.rs` with your impl.
3. Keep the public API identical (same struct name, same method signatures).
4. Every consumer keeps compiling.

Catalyst's contract with the rest of the stack is the **shape** of the service struct, not a trait. Forking preserves the shape — that's the entire extension story.

## Related Specs

- `SPEC_ARCHITECTURE.md` — services layer in the dep graph
- `SPEC_MELTDOWN.md` — error variants used here
- `SPEC_CRANK.md` — retry combinator paired with transient email failures
- `SPEC_FLOWS.md` — services are called from routines, never from flows or transport
