# SPEC_MELTDOWN

Unified error type. Lives in `catalyst::meltdown`. Every fallible Catalyst function returns `Result<T, MeltDown>`.

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
    // Database
    DatabaseConnection,
    DatabaseError,
    RecordNotFound,
    UniqueViolation,
    ForeignKeyViolation,
    CheckViolation,
    NotNullViolation,

    // Auth (opaque bearer tokens, NOT JWT)
    AuthRejected,
    SessionExpired,
    SessionInvalid,
    SessionMissing,
    InsufficientPermissions,

    // Validation / request shape
    ValidationFailed,       // 422, field-level issues
    BadRequest,             // 400, malformed request shape

    // Transport
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    TooManyRequests,        // 429, pairs with retry_after

    // Storage / files
    FileNotFound,
    FilePermissionDenied,
    FileOperationFailed,

    // Serde / config
    SerializationFailed,
    DeserializationFailed,
    ConfigurationError,
    EnvironmentError,

    // External
    ExternalServiceError,

    // Catastrophic fallback
    Unexpected(String),     // renamed from Unknown; treat as TODO marker
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
// Constructor
MeltDown::new(MeltType::ValidationFailed, "email must not be empty")

// Fluent extensions
MeltDown::new(MeltType::RecordNotFound, "user")
    .with_context("table", "users")
    .with_context("id", user_id.to_string())
    .with_source(diesel_err)
    .with_user_message("That user was deleted.")
    .retry_after(Duration::from_secs(5))    // for 429/503
    .mark_transient(true)                    // override default transient-ness
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
    /// True if this error is a transient failure worth retrying.
    /// Default derived from melt_type; overridable via .mark_transient(bool).
    pub fn is_transient(&self) -> bool;

    /// Category: Client | Server | Transient
    pub fn category(&self) -> MeltCategory;

    /// Ergonomic test helper
    pub fn is(&self, t: MeltType) -> bool;
}
```

**Default `is_transient()` mapping:**
- `DatabaseConnection` → true
- `ExternalServiceError` → true
- `TooManyRequests` → true (respects `retry_after`)
- Everything else → false

Used by `Crank` retry combinator to classify errors:

```rust
let result = Crank::new(policy)
    .classify(MeltDown::is_transient)
    .run(|| services::payments::charge(&card, amount))
    .await?;
```

## `From` Impls (conversion)

```rust
impl From<diesel::result::Error> for MeltDown { ... }    // maps DB errors to DB variants
impl From<std::io::Error> for MeltDown { ... }           // FS errors
impl From<std::env::VarError> for MeltDown { ... }       // env reads
impl From<bcrypt::BcryptError> for MeltDown { ... }      // password hashing
// (Add impls for other external error types as they appear.)
```

**NOT present:**
- `impl From<jsonwebtoken::errors::Error>` — JWT killed
- `impl From<&str>` / `impl From<String>` — force explicit typing; no untyped funneling into `Unexpected`
- `impl From<anyhow::Error>` — forces the user to opt into a specific variant

## `IntoResponse` (HTTP)

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
  // ... all variants
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
  if (error.error.context?.field === "email") { /* highlight email input */ }
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
