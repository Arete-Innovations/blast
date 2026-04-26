//! Static file bodies emitted by `auth_scaffold::emit`.
//!
//! Split out from `auth_scaffold.rs` to keep that module focused on
//! orchestration + primer construction; this file is purely the literal
//! text of the scaffolded auth code.
//!
//! Each constant is a complete file; consumers in `auth_scaffold.rs`
//! reference them by name and write them out unmodified (no
//! substitution).

pub const AUTH_FLOW_RS: &str = r#"//! Custom auth flows — register / login / logout / me.
//!
//! These live in `flows/custom/` because auth is bespoke business logic,
//! not generic CRUD. They consume the generated `User` / `Session`
//! structs from `crate::structs::generated` and the generated model
//! helpers from `crate::models::generated`.
//!
//! Replace argon2 with whatever password hash you want — this is custom
//! code, Blast does not regenerate it.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::Engine as _;
use catalyst::ctx::Ctx;
use catalyst::meltdown::{MeltDown, MeltType};
use rand::RngCore;
use serde::{Deserialize, Serialize};

const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 7; // 7 days

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserPublic {
    pub id: i64,
    pub email: String,
    pub role: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginOutput {
    pub token: String,
    pub user: UserPublic,
}

/// Hash a password with argon2id, producing a PHC-encoded string.
pub fn hash_password(plain: &str) -> Result<String, MeltDown> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let phc = argon
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| MeltDown::new(MeltType::Unexpected(format!("argon2 hash: {e}").into()), "register"))?;
    Ok(phc.to_string())
}

/// Verify a plaintext password against a PHC-encoded hash. Returns `Ok(true)`
/// on match, `Ok(false)` on mismatch, `Err` only on parse failures.
pub fn verify_password(plain: &str, phc: &str) -> Result<bool, MeltDown> {
    let parsed = PasswordHash::new(phc)
        .map_err(|e| MeltDown::new(MeltType::Unexpected(format!("argon2 parse: {e}").into()), "login"))?;
    let argon = Argon2::default();
    Ok(argon.verify_password(plain.as_bytes(), &parsed).is_ok())
}

/// Mint an opaque 32-byte session token, base64url-encoded (no padding).
pub fn mint_session_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Current unix epoch seconds.
pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => return 0,
    };
    dur.as_secs() as i64
}

// ── Flow stubs ────────────────────────────────────────────────────────────
//
// The four functions below are the public flow surface. They depend on:
//   - `Ctx` from catalyst (request-scoped: pool, session, services)
//   - The generated `users::*` and `sessions::*` model fns from
//     `crate::models::generated::{users, sessions}` (after `blast gen all`)
//
// Until codegen runs the model fns don't exist; the bodies below are
// therefore left as `todo!()` placeholders that compile against the
// imports declared. The user fills them in (or runs a future
// `blast gen flow auth` once that wizard ships).

pub async fn register(_ctx: &Ctx, input: RegisterInput) -> Result<UserPublic, MeltDown> {
    let _hash = hash_password(&input.password)?;
    // wire to models::generated::users once `blast gen all` has run
    todo!("wire to models::generated::users::insert + return UserPublic projection");
}

pub async fn login(_ctx: &Ctx, input: LoginInput) -> Result<LoginOutput, MeltDown> {
    // 1. fetch user by email; 2. verify_password against user.password_hash;
    // 3. mint_session_token + insert sessions row with TTL; 4. return token.
    let _ = (input, SESSION_TTL_SECS);
    todo!("wire to models::generated::users + models::generated::sessions");
}

pub async fn logout(_ctx: &Ctx) -> Result<(), MeltDown> {
    // pull bearer token from ctx.session, DELETE FROM sessions WHERE token = $1
    todo!("wire to models::generated::sessions");
}

pub async fn me(_ctx: &Ctx) -> Result<UserPublic, MeltDown> {
    // fetch user by ctx.session.user_id, project to UserPublic
    todo!("wire to models::generated::users");
}
"#;

pub const AUTH_HTTP_RS: &str = r#"//! HTTP routes for the custom auth flow.
//!
//! Mounted at `/api/auth/{register, login, logout, me}` by the user app's
//! transport bootstrap. Login + register are public; logout + me require
//! auth (the catalyst session middleware injects `SessionContext` into
//! the request extensions).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use catalyst::ctx::Ctx;
use catalyst::meltdown::MeltDown;
use catalyst::sessions::SessionContext;

use crate::flows::custom::auth::{
    self, LoginInput, LoginOutput, RegisterInput, UserPublic,
};

/// Build the `/api/auth` router. Mount this under `/api` from the
/// transport bootstrap with `.nest("/auth", auth::router(ctx))`.
pub fn router(ctx: Ctx) -> Router {
    Router::new()
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/me", get(me_handler))
        .with_state(ctx)
}

async fn register_handler(
    State(ctx): State<Ctx>,
    Json(body): Json<RegisterInput>,
) -> Result<Json<UserPublic>, AuthErr> {
    let user = auth::register(&ctx, body).await.map_err(AuthErr)?;
    Ok(Json(user))
}

async fn login_handler(
    State(ctx): State<Ctx>,
    Json(body): Json<LoginInput>,
) -> Result<Json<LoginOutput>, AuthErr> {
    let out = auth::login(&ctx, body).await.map_err(AuthErr)?;
    Ok(Json(out))
}

async fn logout_handler(
    State(ctx): State<Ctx>,
    Extension(_session): Extension<SessionContext>,
) -> Result<StatusCode, AuthErr> {
    auth::logout(&ctx).await.map_err(AuthErr)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me_handler(
    State(ctx): State<Ctx>,
    Extension(_session): Extension<SessionContext>,
) -> Result<Json<UserPublic>, AuthErr> {
    let user = auth::me(&ctx).await.map_err(AuthErr)?;
    Ok(Json(user))
}

/// Thin wrapper so MeltDown lands as a proper HTTP response. The full
/// MeltDown -> IntoResponse impl lives in catalyst; this newtype only
/// exists because we want a stable concrete error type at the handler
/// boundary.
pub struct AuthErr(pub MeltDown);

impl IntoResponse for AuthErr {
    fn into_response(self) -> Response {
        self.0.into_response()
    }
}
"#;

pub const SESSION_ADAPTER_RS: &str = r#"//! Concrete `SessionAdapter` impl that joins `sessions` to `users` for
//! catalyst's auth middleware.
//!
//! Wired into the transport bootstrap so axum can populate
//! `Extension<SessionContext>` on every authenticated request.

use async_trait::async_trait;
use catalyst::meltdown::MeltDown;
use catalyst::sessions::{SessionAdapter, SessionUser};
use diesel_async::AsyncPgConnection;

/// Minimal user shape returned by `user_from_token`. The real `User`
/// struct lives in `crate::structs::generated::users::User` after
/// `blast gen all` runs; this shim is what the middleware sees.
#[derive(Clone)]
pub struct AuthUser {
    pub id: i64,
    pub role: String,
}

impl SessionUser for AuthUser {
    fn id(&self) -> i64 {
        self.id
    }
    fn role(&self) -> &str {
        &self.role
    }
}

pub struct AppSessionAdapter;

#[async_trait]
impl SessionAdapter for AppSessionAdapter {
    type User = AuthUser;

    async fn user_from_token(
        &self,
        _conn: &mut AsyncPgConnection,
        _token: &str,
    ) -> Result<Option<Self::User>, MeltDown> {
        // SELECT u.id, u.role FROM sessions s JOIN users u ON
        // u.id = s.user_id WHERE s.token = $1 AND s.expires_at > now_unix()
        // AND u.deleted_at IS NULL.
        todo!("query sessions JOIN users by token");
    }

    async fn create_session(
        &self,
        _conn: &mut AsyncPgConnection,
        _user_id: i64,
    ) -> Result<String, MeltDown> {
        // The auth flow mints + inserts directly so it can set TTL +
        // return the token alongside the user. This implementation is a
        // convenience for callers that go through the trait surface.
        todo!("insert sessions row, return token");
    }

    async fn revoke_session(
        &self,
        _conn: &mut AsyncPgConnection,
        _token: &str,
    ) -> Result<(), MeltDown> {
        // DELETE FROM sessions WHERE token = $1
        todo!("delete sessions row by token");
    }
}
"#;

pub const LOGIN_VUE: &str = r#"<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'

import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Password from 'primevue/password'

import { apiPost } from '@/custom/api/client'
import { setToken } from '@/custom/stores/session'

import PageShell from '@/components/PageShell.vue'

interface LoginResponse {
  token: string
  user: { id: number; email: string; role: string }
}

const router = useRouter()
const email = ref('')
const password = ref('')
const submitting = ref(false)
const error_msg = ref<string | null>(null)

async function on_submit(): Promise<void> {
  submitting.value = true
  error_msg.value = null
  try {
    const res = await apiPost<LoginResponse>('/api/auth/login', {
      email: email.value,
      password: password.value,
    })
    setToken(res.token)
    await router.push('/')
  } catch (err) {
    error_msg.value = err instanceof Error ? err.message : 'login failed'
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <h1>Sign in</h1>
    </template>
    <form class="auth-form" @submit.prevent="on_submit">
      <label class="auth-field">
        <span>Email</span>
        <InputText v-model="email" type="email" autocomplete="email" required />
      </label>
      <label class="auth-field">
        <span>Password</span>
        <Password v-model="password" :feedback="false" toggle-mask required />
      </label>
      <p v-if="error_msg" class="auth-error" role="alert">{{ error_msg }}</p>
      <Button type="submit" label="Sign in" :loading="submitting" />
      <RouterLink to="/register" class="auth-alt">Need an account? Register</RouterLink>
    </form>
  </PageShell>
</template>

<style scoped>
@layer app {
  .auth-form {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-md);
    max-width: 24rem;
    margin-inline: auto;
  }
  .auth-field {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-xs);
  }
  .auth-error {
    color: var(--p-red-500);
  }
  .auth-alt {
    text-align: center;
    color: var(--p-text-muted-color);
  }
}
</style>
"#;

pub const REGISTER_VUE: &str = r#"<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'

import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Password from 'primevue/password'

import { apiPost } from '@/custom/api/client'
import { setToken } from '@/custom/stores/session'

import PageShell from '@/components/PageShell.vue'

interface RegisterResponse {
  id: number
  email: string
  role: string
}

interface LoginResponse {
  token: string
  user: { id: number; email: string; role: string }
}

const router = useRouter()
const email = ref('')
const password = ref('')
const submitting = ref(false)
const error_msg = ref<string | null>(null)

async function on_submit(): Promise<void> {
  submitting.value = true
  error_msg.value = null
  try {
    await apiPost<RegisterResponse>('/api/auth/register', {
      email: email.value,
      password: password.value,
    })
    // Auto-login after register so the user lands authenticated.
    const login = await apiPost<LoginResponse>('/api/auth/login', {
      email: email.value,
      password: password.value,
    })
    setToken(login.token)
    await router.push('/')
  } catch (err) {
    error_msg.value = err instanceof Error ? err.message : 'register failed'
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <h1>Create account</h1>
    </template>
    <form class="auth-form" @submit.prevent="on_submit">
      <label class="auth-field">
        <span>Email</span>
        <InputText v-model="email" type="email" autocomplete="email" required />
      </label>
      <label class="auth-field">
        <span>Password</span>
        <Password v-model="password" toggle-mask required />
      </label>
      <p v-if="error_msg" class="auth-error" role="alert">{{ error_msg }}</p>
      <Button type="submit" label="Create account" :loading="submitting" />
      <RouterLink to="/login" class="auth-alt">Already have an account? Sign in</RouterLink>
    </form>
  </PageShell>
</template>

<style scoped>
@layer app {
  .auth-form {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-md);
    max-width: 24rem;
    margin-inline: auto;
  }
  .auth-field {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-xs);
  }
  .auth-error {
    color: var(--p-red-500);
  }
  .auth-alt {
    text-align: center;
    color: var(--p-text-muted-color);
  }
}
</style>
"#;

pub const SESSION_STORE_TS: &str = r#"// session store — the ONE place token storage happens.
//
// Governor's LocalStorageOutsidePersistence rule treats THIS file as the
// persistence layer; any other file calling localStorage directly is a
// lint violation. Keep token I/O routed through the helpers below.

const TOKEN_KEY = 'catablast.token'

export function getToken(): string | null {
  try {
    return window.localStorage.getItem(TOKEN_KEY)
  } catch (_err) {
    return null
  }
}

export function setToken(token: string): void {
  try {
    window.localStorage.setItem(TOKEN_KEY, token)
  } catch (_err) {
    // Storage unavailable (private mode, quota). Caller treats absence as
    // logged-out — no recovery possible at this layer.
  }
}

export function clearToken(): void {
  try {
    window.localStorage.removeItem(TOKEN_KEY)
  } catch (_err) {
    // ignore
  }
}

export function isAuthed(): boolean {
  return getToken() !== null
}
"#;

pub const API_CLIENT_TS: &str = r#"// api client — minimal fetch wrapper that injects
// `Authorization: Bearer <token>` on every API call. All custom code that
// talks to the backend should route through `apiGet` / `apiPost` rather
// than calling `fetch` directly (Governor's RawFetchOutsideApi rule
// enforces this).

import { getToken, clearToken } from '@/custom/stores/session'

export interface ApiError extends Error {
  status: number
}

function build_headers(extra?: HeadersInit): Headers {
  const headers = new Headers(extra)
  if (!headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json')
  }
  const token = getToken()
  if (token !== null) {
    headers.set('Authorization', `Bearer ${token}`)
  }
  return headers
}

async function handle<T>(res: Response): Promise<T> {
  if (res.status === 401) {
    clearToken()
  }
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText)
    const err = new Error(`HTTP ${res.status}: ${text}`) as ApiError
    err.status = res.status
    throw err
  }
  if (res.status === 204) {
    return undefined as unknown as T
  }
  return (await res.json()) as T
}

export async function apiGet<T>(path: string): Promise<T> {
  const res = await fetch(path, { method: 'GET', headers: build_headers() })
  return handle<T>(res)
}

export async function apiPost<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(path, {
    method: 'POST',
    headers: build_headers(),
    body: JSON.stringify(body),
  })
  return handle<T>(res)
}

export async function apiDelete<T>(path: string): Promise<T> {
  const res = await fetch(path, { method: 'DELETE', headers: build_headers() })
  return handle<T>(res)
}
"#;

pub const AUTH_GUARD_TS: &str = r#"// auth-guard — vue-router beforeEach guard. Routes whose meta declares
// `requiresAuth: true` redirect to `/login` when the session store has
// no token. The guard is registered from the user app's router setup.
//
// The blocking-nav guard installed by `@/router/install-blocking-nav`
// runs separately; this guard is purely about identity. Order does not
// matter: vue-router runs guards in registration order, and this guard
// only consults synchronous local state.

import type { RouteLocationNormalized, NavigationGuardNext, Router } from 'vue-router'

import { isAuthed } from '@/custom/stores/session'

export function installAuthGuard(router: Router): void {
  router.beforeEach(
    (
      to: RouteLocationNormalized,
      _from: RouteLocationNormalized,
      next: NavigationGuardNext,
    ) => {
      const requires_auth = to.matched.some(
        (record) => record.meta && record.meta.requiresAuth === true,
      )
      if (requires_auth && !isAuthed()) {
        next({ path: '/login', query: { redirect: to.fullPath } })
        return
      }
      // Already authed but visiting login/register? Bounce to home.
      if ((to.path === '/login' || to.path === '/register') && isAuthed()) {
        next('/')
        return
      }
      next()
    },
  )
}
"#;
