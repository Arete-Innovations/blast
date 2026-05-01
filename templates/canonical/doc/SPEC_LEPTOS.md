# SPEC_LEPTOS

Frontend stack for Catablast apps. Replaces the legacy Vue/TS/PrimeVue/Vite stack wholesale.

## Stack

| Concern | Choice |
|---------|--------|
| Framework | Leptos 0.7 (Rust → WASM) |
| Render mode | SSR + islands hydration via `cargo-leptos` |
| Router | `leptos_router` (history mode, `<A>` for soft-nav) |
| Component library | `thaw` (typed Leptos components) |
| CSS | scss compiled by cargo-leptos via grass + per-component `.module.scss` via stylance |
| Icons | `icondata` crate (Phosphor feature default) |
| Forms | Hand-rolled thaw inputs + `spawn_local` in `on:submit` + manual `pending`/`last_error` RwSignals (NOT `Action::new_local` — see Mutations section) |
| Tables | `leptos-struct-table` derives on `<R>Public` |
| Page metadata | `leptos_meta` (`Title`, `Meta`, `Link`) |
| Wasm fetch | `gloo-net` |
| Auth token | httpOnly SameSite=Lax cookie (no Secure flag in dev — Firefox drops Secure cookies on plain http://localhost) |

Stack is **locked**. Don't propose Sycamore, Yew, Dioxus, web-awesome, shoelace, tailwind.

## Source-of-truth model

The canonical app crate is **single-crate** with two compilation targets, **no feature flags**. Target-specific dependencies handle the split:

- `cargo build` (host target) → server binary with axum + leptos SSR renderer + diesel + tokio
- `cargo build --target wasm32-unknown-unknown --lib` → WASM bundle for client-side hydration; ssr-only deps don't exist on this target

`cargo-leptos` orchestrates both builds and copies the WASM into `target/site/pkg/`. The server serves `target/site/` as static assets at `/pkg/*`.

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
leptos = { version = "0.7", features = ["ssr"] }
leptos_router = { version = "0.7", features = ["ssr"] }
leptos_meta = { version = "0.7", features = ["ssr"] }
leptos_axum = "0.7"
# server-only crates (diesel, axum, tokio, ...)

[target.'cfg(target_arch = "wasm32")'.dependencies]
leptos = { version = "0.7", features = ["hydrate"] }
leptos_router = "0.7"
leptos_meta = "0.7"
console_error_panic_hook = "0.1"
wasm-bindgen = "0.2"
gloo-net = { version = "0.6", features = ["http"] }
web-sys = { version = "0.3", features = ["Window", "Storage"] }
```

Code-side: `#[cfg(target_arch = "wasm32")]` for wasm-only modules (e.g. `client.rs` with the hydrate entry point); `#[cfg(not(target_arch = "wasm32"))]` for server-only modules (most of the BE). No `feature = "ssr"` / `feature = "hydrate"` cfg attributes anywhere.

## URL topology

Two parallel URL spaces, **no content negotiation**:

- `/foo`, `/foo/42` — Leptos pages (HTML rendered server-side, hydrated client-side)
- `/api/foo`, `/api/foo/42` — REST endpoints (JSON only, plain axum handlers)

Both call the same `flows::*` underneath. Mobile / curl / MCP / arsenal hit `/api/*`. Browser hits bare path.

Per-resource opt-in via Primer per-verb flags `emit_rest_api: bool` and `emit_html_page: bool` (defaults: both true).

## Layered architecture (binding)

Leptos UI lives under `src/transport/leptos/`. Layer rule: pages call `flows::*` and `structs::*` only — same as any other transport handler. `build.rs` `LAYER:11` enforces this.

`TRANSPORT:23` (no `State<Ctx>`) is scoped to `src/transport/http/` — leptos pages use `expect_context::<Ctx>()` (Leptos context system) instead.

```
src/transport/leptos/
├── mod.rs                      barrel
├── app.rs                      <App> root + <Routes> tree + shell()
├── client.rs                   wasm hydrate entry (#[cfg(feature = "hydrate")])
├── api_client.rs               wasm-only HTTP wrapper (emitted by leptos_data codegen pass)
├── auth_storage.rs             cookie-only — no JS storage
├── components/
│   ├── auth_guard.rs           <AuthGuard mode=...> wraps page bodies
│   ├── error_banner.rs         renders MeltDown.message
│   └── page_shell.rs           <PageShell layout=...>
├── pages/
│   ├── welcome.rs / login.rs / register.rs / dashboard.rs / profile.rs / not_found.rs
│   └── generated/              BLAST: per-resource CRUD pages (phase 4)
└── data/                       BLAST + USER: isomorphic data helpers (phase 4)
```

PageLayout + AuthGuardMode enums live in `src/structs/leptos/` per `STRUCTS:22` build-lint.

## Data fetching pattern (locked)

**Isomorphic helpers** with cfg-branched bodies:

```rust
pub async fn load_postari_list(filter: PostariFilter) -> Result<Vec<PostarePublic>, MeltDown> {
    #[cfg(feature = "ssr")]
    {
        let ctx = expect_context::<Ctx>();
        flows::postari::list::run(&ctx, filter).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        crate::transport::leptos::api_client::get_json("/api/postari", &filter).await
    }
}
```

Pages consume via `Resource::new` (queries) and plain `spawn_local` inside `on:submit`/`on:click` for mutations (not `Action::new_local` — see Mutations section for why).

- **SSR-side**: zero HTTP roundtrip. Page renders with data baked into the HTML payload via Leptos's hydration handoff.
- **Client-side**: when dependencies change (URL params, filters), wasm re-fetches via `/api/<r>`.

`MeltDown` and projection structs need `Serialize + Deserialize` so the SSR result can travel to wasm.

## Auth (locked)

httpOnly SameSite=Lax cookie (no Secure flag in dev — Firefox + recent Chrome drop Secure cookies on plain http://localhost). Server reads cookie on SSR request, knows session immediately, can render full page or redirect. Wasm has no JS access (httpOnly). All `/api/*` calls send cookie automatically.

**Wire shape:** `/api/auth/{login,register,me}` all return `SessionContext` (`{session_id, user_id, role}`) directly. `SessionContext.token` carries `#[serde(skip_serializing, default)]` so the cookie token never leaks into the JSON body. The httpOnly cookie is the only place the token lives client-side.

**Auth middleware behavior on stale cookies (binding):** when the request carries a session cookie that no longer resolves (DB row gone, expired, malformed), the middleware MUST swallow the resolve error, log Debug, and fall through to anonymous Ctx. It MUST NOT propagate `SessionInvalid` to the response — doing so 401's `/api/auth/login` itself, making it impossible to recover from a stale cookie. See `src/transport/http/middleware/auth.rs::request_ctx_middleware`.

`AuthGuard` component wraps every protected page. It reads from a global `SessionStore` signal (typed wrapper around `RwSignal<Option<SessionContext>>` defined at `src/structs/leptos/session_store.rs`). The store is provided in `<App>` via `provide_session_store()` and hydrated by an `Effect::new` that calls `load_session()` on mount. On SSR-side first render, the store starts empty (`None`); on wasm hydrate, the Effect calls `/api/auth/me` via `api_client::get_json` and populates the store. Renders `<Redirect path="/login"/>` if blocked.

**Known limitation (deferred to phase 12)**: SSR-side first render shows protected pages briefly as if unauthed (then wasm corrects via the load_session Effect). The clean fix is `leptos_routes_with_context` callback that reads cookie from request headers, resolves session, and `provide_context::<Option<SessionContext>>` per-request — eliminating the flash. Currently not implemented.

```rust
#[component]
pub fn DashboardPage() -> impl IntoView {
    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Cards>
                ... content ...
            </PageShell>
        </AuthGuard>
    }
}
```

Hand-written and codegen'd pages use the **same primitive**.

## Layouts

`PageShell` accepts `layout: PageLayout`:

| Layout | Use case |
|--------|----------|
| `Cards` | Default. Stacked card sections. |
| `Split` | Master-detail. List on left, detail on right. |
| `Table` | Full-bleed data table with own toolbar. |
| `Bleed` | Maps, canvases, full-screen viz. |
| `Tabbed` | Tab container; child tabs pick own layout. |

Layout owns spacing. PageShell does not accept `padding`/`margin`/`gap`/`width` props.

## Mutations

Optimistic updates are **banned** (carries forward governor rule). Pattern:
1. User action → `on:submit` handler `spawn_local`s the future
2. Wasm awaits server response
3. On success: set the relevant signals (e.g. `session_store.set(...)`) → toast success → `navigate(...)` if applicable
4. On error: `err.log()` → toast `MeltDown.message` byte-for-byte from server → optionally stash in a `last_error` signal for an inline `<ErrorBanner/>`

Toast singleton signal in `src/transport/leptos/signals/toast.rs`, rendered by `<ToastHost/>` in `<App>`.

**Why not `Action::new_local`?** The combo `Action::new_local(...)` + `Effect::new(move |_| match action.value().get() { Some(Ok(out)) => navigate(...) })` deadlocks the wasm event loop on submit: navigate's reactive flush unmounts the page mid-Effect, leaving the runtime in a recursive state. We learned this the hard way (see WIP doc, "Hard lessons"). Plain `spawn_local` runs the future at microtask time, no flush re-entry.

## Form submission

Forms require JS hydration (no `<noscript>` fallback). Pattern (hand-written + codegen'd):

```rust
let pending = RwSignal::new(false);
let last_error: RwSignal<Option<MeltDown>> = RwSignal::new(None);
let navigate = StoredValue::new_local(use_navigate());
// ^ StoredValue wrap because use_navigate() returns `impl Fn + Clone` (no Copy);
//   capturing-by-move makes the on_submit closure FnOnce, which view! rejects.

let on_submit = move |ev: leptos::ev::SubmitEvent| {
    ev.prevent_default();
    if pending.get_untracked() { return; }
    // optional client-side validation (validate_<r>_* fns from validators codegen)
    pending.set(true);
    last_error.set(None);
    let input = /* parse RwSignal values into typed <R>Insertable */;
    spawn_local(async move {
        let result = do_<r>_create(input).await;
        pending.set(false);
        match result {
            Ok(out) => {
                toasts.success("...");
                navigate.with_value(|nav| nav("/somewhere", Default::default()));
                // or call refetch on the parent Resource
            }
            Err(err) => {
                err.log();
                toasts.error(format!("{}", err));
                last_error.set(Some(err));
            }
        }
    });
};
```

Codegen'd forms (`<R>CreateForm`/`<R>EditForm`) follow the same pattern with thaw inputs (`<Input/>`, `<Checkbox/>`, `<Combobox/>` for enums) and `validate_<r>_*` called BEFORE the spawn_local dispatch.

No `#[server]` macro use. Plain axum handlers + Leptos pages, both calling the flow.

## Validators

Single Rust source (`src/structs/generated/validators/<r>.rs`). Compiles to both server binary and wasm because the whole crate compiles to both. Same fn called from REST handlers AND from form components.

See `SPEC_VALIDATORS.md`.

## CSS

`style/main.scss` is the entry; uses `tokens.scss` (design tokens) and `base.scss` (reset + root font scaling). Cargo-leptos compiles via `grass` to `target/site/pkg/canonical.css`.

Per-component `.module.scss` files use `stylance` for hashed-classname scoping. No global styles outside `tokens.scss` / `base.scss`.

OKLCH color, semantic class names, no hex outside theme overrides, no inline styles. Lint rules carry forward into a `LEPTOS:N` build.rs lint family.

## WebSocket (Relay)

Server-side `transport/ws/` unchanged from pre-leptos design. Client-side replaces TS `client.ts` with `gloo-net::websocket`. Subscription primitive: `use_topic<T>(topic: &str) -> ReadSignal<Option<T>>` lives in `src/transport/leptos/signals/relay.rs`.

Auth handshake via cookie on WS upgrade (cookie sent automatically by browser on same-origin upgrade). Server reads cookie during upgrade handler, loads session, attaches to relay state.

## Page metadata

Per-page `<Title text="..."/>` via `leptos_meta`. `<MetaTags/>` mounted in `<App>` `<head>` slot. OG tags + favicons via `leptos_meta::Meta` and `Link`.

## Dark mode

OS preference default + manual toggle. Thaw `<ConfigProvider theme>` driven by signal. Toggle persists in cookie (server reads on next SSR for no light-flash).

## i18n

Out of scope.

## End-to-end testing

Standalone `e2e/` workspace member ships with the canonical template. Drives a headless Firefox via geckodriver + fantoccini (rust webdriver) + cookie-aware reqwest. Boots `cargo leptos serve`, waits for `/api/healthz`, then exercises register → logout → login → `/api/auth/me` round-trip.

Wire-up:
- `[package.metadata.leptos] end2end-cmd = "cargo run --release"` + `end2end-dir = "e2e"` in canonical's Cargo.toml.
- `cargo leptos end-to-end` (or `blast e2e`) builds + serves + runs the e2e binary.
- pre-req: `pacman -S geckodriver firefox` (or chromedriver — fantoccini handles either; default in this template is geckodriver since Firefox is the primary dev browser).

The e2e binary spawns its own geckodriver on port 4444 in a subprocess and kills it on exit. It does NOT touch the user's running Firefox — geckodriver creates an isolated profile per session. Headless mode via `moz:firefoxOptions.args = ["-headless"]`.

Diagnostic patterns baked into the e2e (kept in for future debugging):
- `setTimeout(...click(), 50)` instead of `client.click()` so we can poll the JS thread post-submit without WebDriver waiting for page-settled.
- `tokio::time::timeout(2s, client.execute("return Date.now()"))` poll loop — if it doesn't return, the wasm hydrate has wedged the event loop (regression check for the `Action::new_local + Effect::new(action.value())` deadlock pattern).
- fetch hook (`window.fetch = wrapper`) records all outbound XHRs so we can assert the right `/api/*` endpoint was hit even if the resulting navigate happens too fast to observe.
- per-step `tokio::time::timeout` of `STEP_TIMEOUT` (40s) wrapping each phase. Master timeout at 120s to guarantee the suite never hangs the runner.

## Related specs

- `SPEC_ARCHITECTURE.md` — layer rules
- `SPEC_PRIMER.md` — per-resource state file
- `SPEC_VALIDATORS.md` — single-source validator codegen
- `SPEC_FLOWS.md` — flow authoring
- `SPEC_MELTDOWN.md` — error type + wire shape
- `SPEC_RELAY.md` — WS protocol
- `SPEC_SESSIONS.md` — auth tokens (cookie transport)
- `blast/doc/SPEC_CODEGEN.md` — what Blast emits for the leptos UI
