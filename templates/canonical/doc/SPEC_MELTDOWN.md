# SPEC_MELTDOWN

Unified error type. Lives in `crate::meltdown`. Returned by flows, routines, models, and services — `Result<T, MeltDown>` everywhere in those layers. `IntoResponse` is implemented in `transport/http/` only; inner layers return `MeltDown` values and never touch HTTP concerns.

## Shape

Struct carrying a categorized kind + rich context.

```rust
pub struct MeltDown {
    pub melt_type: MeltType,
    pub details: String,
    pub user_message: Option<String>,
    pub context: Option<HashMap<String, String>>,
    pub retry_after: Option<Duration>,
    pub transient: bool,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}
```

## `MeltType` Variants

```rust
pub enum MeltType {

    DatabaseConnection,
    DatabaseError,
    RecordNotFound,
    UniqueViolation,
    ForeignKeyViolation,
    CheckViolation,
    NotNullViolation,


    AuthRejected,
    SessionExpired,
    SessionInvalid,
    SessionMissing,
    InsufficientPermissions,


    ValidationFailed,
    BadRequest,


    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    TooManyRequests,


    FileNotFound,
    FilePermissionDenied,
    FileOperationFailed,


    SerializationFailed,
    DeserializationFailed,
    ConfigurationError,
    EnvironmentError,


    ExternalServiceError,


    Unexpected(String),
}
```

## HTTP Status Mapping

| Variant | Status |
|---------|--------|
| `AuthRejected`, `SessionExpired`, `SessionInvalid`, `SessionMissing`, `Unauthorized` | 401 |
| `InsufficientPermissions`, `Forbidden`, `FilePermissionDenied` | 403 |
| `ValidationFailed` | 422 |
| `BadRequest`, `CheckViolation`, `NotNullViolation` | 400 |
| `NotFound`, `RecordNotFound`, `FileNotFound` | 404 |
| `MethodNotAllowed` | 405 |
| `UniqueViolation`, `ForeignKeyViolation` | 409 |
| `TooManyRequests` | 429 |
| `ExternalServiceError` | 503 |
| Everything else | 500 |

## Builder API

```rust

MeltDown::new(MeltType::ValidationFailed, "email must not be empty")


MeltDown::new(MeltType::RecordNotFound, "user")
    .with_context("table", "users")
    .with_context("id", user_id.to_string())
    .with_source(diesel_err)
    .with_user_message("That user was deleted.")
    .retry_after(Duration::from_secs(5))
    .mark_transient(true)
```

## Named Constructors

```rust
MeltDown::db_connection("could not acquire pool")
MeltDown::record_not_found("user")
MeltDown::unique_violation("email")
MeltDown::auth_rejected()
MeltDown::session_expired()
MeltDown::session_invalid(token_prefix)
MeltDown::session_missing()
MeltDown::insufficient_permissions()
MeltDown::validation_failed("password too short")
MeltDown::bad_request("expected JSON body")
MeltDown::too_many_requests(Duration::from_secs(60))
```

## Classification Methods

```rust
impl MeltDown {


    pub fn is_transient(&self) -> bool;


    pub fn category(&self) -> MeltCategory;


    pub fn is(&self, t: MeltType) -> bool;
}
```

**Default `is_transient()` mapping:**
- `DatabaseConnection` → true
- `ExternalServiceError` → true
- `TooManyRequests` → true (respects `retry_after`)
- Everything else → false

Used by `Crank` retry combinator to classify errors. Crank is called from flows wrapping routine calls — never from services (which are single-shot) or transport (which calls flows directly):

```rust
// inside a flow
let result = Crank::new(policy)
    .classify(MeltDown::is_transient)
    .run(|| routines::payments::attempt_charge(&ctx, &card, amount))
    .await?;
```

## `From` Impls (conversion)

```rust
impl From<diesel::result::Error> for MeltDown { ... }
impl From<std::io::Error> for MeltDown { ... }
impl From<std::env::VarError> for MeltDown { ... }
impl From<bcrypt::BcryptError> for MeltDown { ... }

```

**NOT present:**
- `impl From<jsonwebtoken::errors::Error>` — JWT killed
- `impl From<&str>` / `impl From<String>` — force explicit typing; no untyped funneling into `Unexpected`
- `impl From<anyhow::Error>` — forces the user to opt into a specific variant

## `IntoResponse` (HTTP)

Lives in `transport/http/` only. Inner layers (flows, routines, models, services) return `MeltDown` values — they have no dependency on Axum response types.

```rust
impl IntoResponse for MeltDown {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = json!({
            "error": {
                "code": status.as_u16(),
                "type": self.melt_type_str(),
                "message": self.user_message(),
                "context": self.context,
            }
        });

        let mut response = (status, Json(body)).into_response();

        if let Some(retry) = self.retry_after {
            response.headers_mut().insert(
                "Retry-After",
                retry.as_secs().to_string().parse().unwrap(),
            );
        }

        response
    }
}
```

Response envelope is always:

```json
{
  "error": {
    "code": 422,
    "type": "ValidationFailed",
    "message": "Email must not be empty.",
    "context": { "field": "email" }
  }
}
```

## Logging

**MeltDown does NOT log itself.** Logging is a middleware concern.

A tower middleware in `transport/http/` inspects response status:
- 4xx → log at `warn` level with the MeltDown body
- 5xx → log at `error` level with the MeltDown body + source chain

This prevents double-logging (previous implementation logged inside `into_response` AND inside `From<MeltDown> for ApiError`).

## TS Codegen

Blast emits `frontend/src/generated/types/meltdown.ts` — a const enum matching `MeltType` variants so FE can match on type:

```ts
export const MeltType = {
  DatabaseConnection: "DatabaseConnection",
  ValidationFailed: "ValidationFailed",
  UniqueViolation: "UniqueViolation",

} as const;
export type MeltType = (typeof MeltType)[keyof typeof MeltType];

export type MeltDownResponse = {
  error: {
    code: number;
    type: MeltType;
    message: string;
    context?: Record<string, string>;
  };
};
```

Used in FE composables:

```ts
const { error } = await api.users.create(input);
if (error?.error.type === MeltType.UniqueViolation) {
  if (error.error.context?.field === "email") {  }
}
```

Regenerated every time `MeltType` changes in Rust. Never hand-maintained.

## Anti-Patterns

- **Stuffing anything into `Unexpected`** — every `Unexpected(...)` is a TODO for "this error should have a variant." Use it as a forcing function to add specific variants, not a dumping ground.
- **Custom per-flow error enums** — no. One app-wide `MeltDown`. Let `#[from]` conversions bridge external error types. Keeps the response surface uniform.
- **Logging inside `MeltDown`** — no. Middleware logs.
- **Hand-writing MeltDown at FE** — no. FE imports the generated TS enum.
- **Adding HTMX/template variants** — no. HTMX was killed. If a future render path needs a variant, add it deliberately.

## Migration From Current `src/meltdown.rs`

Current file has stale variants. Cleanup on next touch:

- Drop `ExpiredToken` (duplicate of `TokenExpired`)
- Drop `InvalidToken`, `MissingToken`, `TokenExpired` → rename to `SessionInvalid`, `SessionMissing`, `SessionExpired`
- Drop `InvalidCredentials` → rename to `AuthRejected`
- Drop `InvalidInput`, `MissingField` → use `ValidationFailed` with context
- Drop `Unknown` → rename to `Unexpected(String)`
- Drop `TemplateRenderFailed` (HTMX/Tera fossil)
- Drop `From<&str>`, `From<String>`, `From<jsonwebtoken::errors::Error>` impls
- Drop `.log()` call inside `IntoResponse` (move to tower middleware)
- Add `TooManyRequests` variant
- Add `retry_after: Option<Duration>` field
- Add `transient: bool` field + `is_transient()`, `category()`, `is()` methods
- Emit `frontend/src/generated/types/meltdown.ts` from Blast

## Related Specs

- `SPEC_CRANK.md` — retry classifier uses `MeltDown::is_transient()`
- `SPEC_SESSIONS.md` — auth variants used by session middleware
- `SPEC_FRONTEND.md` — FE consumes the TS enum
- `SPEC_ARCHITECTURE.md` — where MeltDown lives in the stack
