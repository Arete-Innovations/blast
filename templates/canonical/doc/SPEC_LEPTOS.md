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
| Forms | `leptos-form` derives on `<R>Insertable` / `<R>Patch` |
| Tables | `leptos-struct-table` derives on `<R>Public` |
| Page metadata | `leptos_meta` (`Title`, `Meta`, `Link`) |
| Wasm fetch | `gloo-net` |
| Auth token | httpOnly secure SameSite=Strict cookie |

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
├── api_client.rs               wasm-only HTTP wrapper (planned phase 4)
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

Pages consume via `Resource::new` (queries) and `Action::new` (mutations).

- **SSR-side**: zero HTTP roundtrip. Page renders with data baked into the HTML payload via Leptos's hydration handoff.
- **Client-side**: when dependencies change (URL params, filters), wasm re-fetches via `/api/<r>`.

`MeltDown` and projection structs need `Serialize + Deserialize` so the SSR result can travel to wasm.

## Auth (locked)

httpOnly secure SameSite=Strict cookie. Server reads cookie on SSR request, knows session immediately, can render full page or redirect. Wasm has no JS access (httpOnly). All `/api/*` calls send cookie automatically.

`AuthGuard` component wraps every protected page. On SSR side, it reads `Option<SessionContext>` from Leptos context (provided per-request by the route handler that decoded the cookie). Renders `<Redirect path="/login"/>` if blocked.

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
1. User action → `Action::dispatch(input)`
2. Wasm awaits server response
3. On success: refetch invalidating Resource → render fresh data → toast success
4. On error: toast `MeltDown.message` byte-for-byte from server

Toast singleton signal in `src/transport/leptos/signals/toast.rs`, rendered by `<ToastHost/>` in `<App>`.

## Form submission

Forms require JS hydration (no `<noscript>` fallback). Hand-written forms use `<form on:submit=...>` + `Action::new`. Codegen'd forms use `leptos-form` derives on `<R>Insertable`/`<R>Patch` — derive emits `<R>CreateForm`/`<R>EditForm` components that wrap an Action and render thaw inputs.

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

## Related specs

- `SPEC_ARCHITECTURE.md` — layer rules
- `SPEC_PRIMER.md` — per-resource state file
- `SPEC_VALIDATORS.md` — single-source validator codegen
- `SPEC_FLOWS.md` — flow authoring
- `SPEC_MELTDOWN.md` — error type + wire shape
- `SPEC_RELAY.md` — WS protocol
- `SPEC_SESSIONS.md` — auth tokens (cookie transport)
- `blast/doc/SPEC_CODEGEN.md` — what Blast emits for the leptos UI
