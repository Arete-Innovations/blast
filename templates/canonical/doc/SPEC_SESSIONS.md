# SPEC_SESSIONS

Opaque bearer token auth. Dual transport: cookies for web, `Authorization: Bearer` for mobile/API. Single DB-backed source of truth.

NOT JWT. Chosen explicitly over JWT because:
- Revocation is instant (delete a row)
- Mid-session permission changes propagate immediately
- No refresh token choreography
- No algorithm-confusion / key-rotation footguns
- Claim staleness isn't a problem
- Token payload isn't base64-visible to the client

## DB Table

```sql
CREATE TABLE sessions (
    id              BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      BYTEA NOT NULL UNIQUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    user_agent      TEXT,
    ip              INET,
    revoked         BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX sessions_token_hash_idx ON sessions (token_hash) WHERE NOT revoked;
CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at) WHERE NOT revoked;
```

Token shape: `cb_<32 random base58 chars>` (or similar). The raw token is shown to the client once at creation. Only the SHA-256 hash is persisted.

## Token Issuance Flow

Flows call routines only. The login flow delegates credential verification and session
creation to routines; it never touches models or services directly.

```rust

pub async fn run(ctx: &Ctx, input: LoginInput) -> Result<LoginOutcome, MeltDown> {
    ctx.require_anonymous()?;

    let user = routines::users::find_by_credentials(ctx, &input.email, &input.password).await?;

    let outcome = routines::sessions::create(ctx, &user, &input.meta).await?;

    Ok(outcome)
}
```

The routines handle the model + service calls:

```rust
// routines/users.rs
pub async fn find_by_credentials(ctx: &Ctx, email: &str, password: &str) -> Result<User, MeltDown> {
    let user = models::users::find_by_email(ctx.conn(), email).await?
        .ok_or_else(MeltDown::auth_rejected)?;
    if !services::crypto::verify_password(password, &user.password_hash)? {
        return Err(MeltDown::auth_rejected());
    }
    Ok(user)
}

// routines/sessions.rs
pub async fn create(ctx: &Ctx, user: &User, meta: &RequestMeta) -> Result<LoginOutcome, MeltDown> {
    let raw_token = services::crypto::generate_session_token();
    let token_hash = services::crypto::sha256(&raw_token);
    let expires_at = Utc::now() + Duration::from_days(30);
    models::sessions::insert(ctx.conn(), &NewSession {
        user_id: user.id,
        token_hash,
        expires_at,
        user_agent: meta.user_agent.clone(),
        ip: meta.ip,
    }).await?;
    Ok(LoginOutcome { token: raw_token, user: user.into_public(), expires_at })
}
```

## Transport Integration

### Web (cookie)

After login, transport sets an httpOnly secure cookie:

```rust

pub async fn login(State(ctx), Json(input)) -> Result<impl IntoResponse, MeltDown> {
    let outcome = flows::auth::login::run(&ctx, input).await?;
    let cookie = Cookie::build(("cb_session", outcome.token))
        .http_only(true)
        .secure(ctx.env == Env::Prod)
        .same_site(SameSite::Lax)
        .max_age(outcome.expires_at - Utc::now())
        .path("/")
        .build();
    Ok((jar.add(cookie), Json(UserPublic::from(outcome.user))))
}
```

### Mobile / API (Bearer)

Login route for API clients returns the token in the body:

```json
{
  "token": "cb_abc...",
  "user": { ... },
  "expires_at": "2026-05-24T00:00:00Z"
}
```

Client stores in Keychain / Keystore, sends on every request:

```
Authorization: Bearer cb_abc...
```

## Middleware

Middleware loads the bearer token (if present) and constructs a `Ctx`. It does NOT enforce
authorization — that is the flow's responsibility via `ctx.require_anonymous()` /
`ctx.require_role(...)`.

- No token → anonymous `Ctx` (no session, no user).
- Valid token → session-loaded `Ctx` with `ctx.session` populated.
- Invalid / expired / revoked token → `MeltDown::session_invalid` / `session_expired` (hard
  reject at the boundary — malformed credential is not an anonymous request).

```rust


pub async fn session_ctx<B>(
    State(pool): State<PgPool>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, MeltDown> {
    let raw_token = match extract_token(req.headers(), req.cookies()) {
        None => {
            req.extensions_mut().insert(Ctx::anonymous(pool));
            return Ok(next.run(req).await);
        }
        Some(t) => t,
    };

    let token_hash = crypto::sha256(&raw_token);
    let session = models::sessions::find_by_hash(&pool, &token_hash).await?
        .ok_or_else(MeltDown::session_invalid)?;

    if session.revoked {
        return Err(MeltDown::session_invalid());
    }
    if session.expires_at < Utc::now() {
        return Err(MeltDown::session_expired());
    }

    models::sessions::touch(&pool, session.id).await.ok();

    let user = models::users::find(&pool, session.user_id).await?
        .ok_or_else(MeltDown::session_invalid)?;

    req.extensions_mut().insert(Ctx::authenticated(pool, Session { id: session.id, user }));
    Ok(next.run(req).await)
}
```

Apply globally (not per-route). Every handler receives a `Ctx`; flows decide what auth is required.

`extract_token` checks:
1. `Authorization: Bearer <token>` header
2. `cb_session` cookie

First match wins. Supports both transports uniformly.

## Revocation

### Single session

Flows call a routine; the routine owns the model call:

```rust
// inside flows/auth/logout.rs
routines::sessions::revoke(ctx, ctx.session().id).await?;
```

Transport clears the cookie:
```rust
let jar = jar.add(Cookie::build(("cb_session", ""))
    .max_age(Duration::ZERO)
    .path("/")
    .build());
```

### All sessions for a user

```rust
// inside a flow (e.g. flows/auth/revoke_all_sessions.rs)
routines::sessions::revoke_all_for_user(ctx, user_id).await?;
```

Use case: password changed, user reports device lost, role revoked.

### Auto-prune expired

A Fuse runs periodically:

```rust
Fuse::named("prune_expired_sessions")
    .schedule(Schedule::every(Duration::from_hours(1)))
    .run(flows::sessions::prune_expired::run)
```

```sql
DELETE FROM sessions WHERE expires_at < NOW() - INTERVAL '7 days';
```

Or just mark `revoked = true`; keep rows for audit. Policy decided per app.

## Rotation (Optional)

Sliding expiration: on each request, extend `expires_at` if `last_seen_at` is older than threshold. Keeps active users logged in indefinitely, inactive ones time out.

Rotation (new token issued): less important since tokens are opaque + DB-validated; no reason to rotate unless leaked. Compromised token gets revoked, not rotated.

## WebSocket Auth

Same middleware on WS upgrade:

```rust

let app = Router::new()
    .route("/ws", get(ws_upgrade_handler))
    .layer(middleware::from_fn(session_ctx));
```

Session attached to `WsSession.ctx`. Used by `Relay` subscription auth (`can_subscribe`).

## API Key Variant (future)

For programmatic / machine-to-machine access, a separate table (same shape as `sessions` but named `api_keys`, with `user_id` possibly null and `scopes` column) may be added. Same opaque token model, same SHA-256 hashing, same middleware pattern with a different table. Pending design.

## Anti-Patterns

**JWT:**
Killed explicitly. Don't reintroduce.

**Storing raw tokens in DB:**
```sql

CREATE TABLE sessions (token TEXT ...);
```

Always hash. If DB is compromised, plain tokens = account takeover. Hashed tokens = useless without the originals.

**Token in URL:**
```
GET /api/orders?token=cb_abc...
```

Tokens leak via referrer headers, logs, analytics. Always in cookie or Authorization header.

**Over-long sessions:**
30-day default. For high-risk apps, shorter. Don't issue 1-year sessions unless the threat model explicitly supports it.

**Regenerating token every request:**
Unneeded noise. Tokens are opaque and validated against DB; no reason to rotate per request.

## Related Specs

- `SPEC_MELTDOWN.md` — `SessionMissing`/`SessionExpired`/`SessionInvalid` variants
- `SPEC_FLOWS.md` — login/logout flow shape
- `SPEC_RELAY.md` — WS auth reuses this middleware
- `SPEC_CONFIG.md` — `SESSION_SIGNING_KEY` declared in `app.ron` env var spec
