# SPEC_LEPTOS

Frontend stack for Catablast apps. Replaces the legacy Vue/TS/PrimeVue/Vite stack wholesale.

## Stack

| Concern | Choice |
|---------|--------|
| Framework | Leptos 0.7 (Rust → WASM) |
| Render mode | SSR + islands hydration via `cargo-leptos` |
| Router | `leptos_router` (history mode, `<A>` for soft-nav) |
| Component library | `thaw` is in deps but **NOT used by codegen** — its widgets call `wasm-bindgen` statics that panic on SSR (`js-sys-0.3.97 cannot access imported statics on non-wasm targets`). Generated forms/pages emit **native HTML** (`<input>`, `<select>`, `<textarea>`, `<form>`, `<p>`) with `prop:value` + `on:input`/`on:change` bindings. Hand-rolled wasm-only components MAY use thaw. |
| CSS | scss compiled by cargo-leptos via grass + per-component `.module.scss` via stylance |
| Icons | `icondata` crate (Phosphor feature default) |
| Forms | Hand-rolled native HTML inputs + `spawn_local` in `on:submit` + manual `pending`/`last_error` RwSignals (NOT `Action::new_local` — see Mutations section) |
| Data fetching (pages) | `RwSignal<Option<Result<T, MeltDown>>>` + `#[cfg(target_arch = "wasm32")] Effect::new(spawn_local(load_*))` — NOT `Resource::new`/`LocalResource::new` (both pull js-sys statics on SSR). SSR renders `<p>"Loading..."</p>` placeholder; wasm hydrate fires Effect → fetch → render. |
| Tables | Native `<table>/<thead>/<tbody>` rendered via `<For>` over a codegen-emitted `<R>TableRow` (display-safe subset of `<R>Public` — Jsonb/Bytea/Numeric stripped). No third-party table crate (leptos-struct-table requires leptos 0.8 since v0.15; the only leptos-0.7-compat version was a broken beta). |
| Page metadata | `leptos_meta` (`Title`, `Meta`, `Link`) |
| Wasm fetch | `gloo-net` |
| Auth token | httpOnly SameSite=Lax cookie (no Secure flag in dev — Firefox drops Secure cookies on plain http://localhost) |
| Session boot | `<script id="cata-session-boot">window.__cata_session = {SessionContext-JSON}</script>` injected by SSR shell from per-request Ctx; wasm reads synchronously via `js_sys::Reflect::get + JSON.stringify + serde_json::from_str` BEFORE first render. SSR + hydrate agree on `session: Option<SessionContext>` at first paint. No `/api/auth/me` round-trip on hydrate. |

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

### Path = identity

The path identifies the resource being viewed: `/posts` is the post list, `/posts/42` is post 42, `/posts/new` is the create form. The path NEVER carries view state (no `/posts/page-2`, no `/posts/sort=created_at`). One path, one canonical "what am I looking at."

### Query = view state

The query string carries everything else: pagination, sort, filters, open dialogs. Two helpers in `crate::transport::leptos::signals` keep components honest:

- `use_url_list_state() -> UrlListState` — bidirectional sync between URL and `RwSignal`s for `page`, `page_size`, `sort`, `filter` per the wire schema (`?page=2&page_size=25&sort=-created_at&filter[col]=val`). `to_list_query()` snapshots the state into a `ListQuery` for data loaders. Codegen wires this into every list page automatically: the loader-Effect re-fires whenever any URL-state signal changes.
- `use_query_dialog(name) -> QueryDialog` — modal/dialog state lives in the URL, not in component-local signals. `?dialog=<name>` toggles visibility; optional `?dialog_id=<n>` carries a row id (e.g. for edit/delete confirms). `QueryDialog::open(id)` and `QueryDialog::close()` mutate via `use_navigate` + replace. This makes dialogs bookmarkable and survives reload.

Mutations to either helper push via `NavigateOptions { replace: true, scroll: false }` so filter typing / paginating doesn't spam history with a new entry per keystroke.

### Gray-zone heuristic

When deciding whether a piece of state belongs in the URL (query) or in a component-local signal, ask: **if a user bookmarks this URL and reopens it tomorrow, will they see the same content AND the same UI affordances open?**

- "filter set to active=true" → yes, belongs in URL.
- "edit dialog open for row 42" → yes, belongs in URL (`?dialog=edit&dialog_id=42`).
- "user is hovering a button" → no, transient.
- "form has unsaved input" → no, transient (and we don't want bookmark-rehydration confusion).

If the answer is yes, use one of the URL helpers. If no, plain `RwSignal`. Never spread the same state across both layers.

## Blocking navigation

Hand-rolled and codegen'd pages call `use_blocking_navigate()` (NOT `use_navigate()` directly) for any user-driven nav (login redirect, post-create nav-to-detail, post-delete nav-to-list). The wrapper drives a global `<NavProgress/>` bar mounted at the top of the viewport so the user always sees that the route is in flight.

```rust
let navigate = StoredValue::new_local(use_blocking_navigate());
// ... inside on:submit:
navigate.with_value(|nav| nav("/dashboard"));
// (note: single-arg signature; NavigateOptions defaulted internally)
```

**Lifecycle (locked):**
1. **Click** → `NavState::Pending(start_at_ms)` set, target stashed in store, inner `use_navigate()` fires immediately. The progress bar fills 0 → 100% animated over a **500ms budget**.
2. **Budget exceeded** → animation pivots to indeterminate pulse. The store does NOT cancel or change state; the visual just signals "still working" without committing to a known finish line.
3. **Settle** → `<NavProgress/>` watches `use_location().pathname`; when it matches the stashed target, store flips `Pending → Settled`, bar fades opacity 1 → 0 over 200ms.
4. **Reset** → `setTimeout(200ms)` flips `Settled → Idle`, target cleared, bar removed from layout.

**Limitation (binding) — leptos 0.7 has no real blocking-load:** the framework does not expose nav-lifecycle events (start / pending-data-loaded / committed). `Resource::new` / `LocalResource::new` could in theory provide blocking-on-data-resolved gates, but the data-fetching pattern locked above (RwSignal + cfg-gated Effect) explicitly bypasses them. The `<NavProgress/>` settle signal is therefore **the URL change itself**, not the destination's data being ready. This means: for routes whose page-body Effect kicks off a slow `/api/*` fetch, the bar settles when the route mounts, NOT when the data lands. The destination's own `<p>"Loading..."</p>` placeholder takes over from there. This is a best-effort settle-via-pathname; proper blocking nav awaits framework support.

**Position:** the bar is rendered inside `<Router>` via `<NavProgress/>` next to `<ToastHost/>` in `app.rs`. CSS lives in `style/base.scss` (`.nav-progress`, `.nav-progress--idle/--pending/--settled` + `@keyframes`) — kept out of `transport/leptos/` to satisfy `LEPTOS:1` (no inline `style=`) and `LEPTOS:3` (no raw `px`). Color: `var(--app-color-brand)`. Height: `0.125rem`. Z-index: above all page content.

**Cancellation:** none. The store does not track in-flight task handles. If the user clicks a second nav while one is pending, the second call overwrites the target and start time; the old "pending" timeline is dropped silently. Leptos 0.7 task cancellation is limited and the simpler model has been adequate.

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

**Isomorphic helpers** with target-arch-cfg branched bodies. Codegen emits these into `src/transport/leptos/data/generated/<r>.rs`:

```rust
pub async fn load_posts_list(query: ListQuery) -> Result<ListResponse<PostPublic>, MeltDown> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ctx = expect_context::<Ctx>();
        crate::flows::generated::posts::list::run(&ctx, query).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        let path = format!("/api/posts?{}", query_to_query_string(&query));
        crate::transport::leptos::api_client::get_json(&path).await
    }
}
```

The `RwSignal<Option<Result<T, MeltDown>>>` + wasm-only `Effect` pattern is now wrapped in three reactivity tiers in `src/transport/leptos/signals/reactivity.rs`. All three return the same signal cell shape — pages consume the value identically; only the lifecycle differs.

### Tier 1 — Static (`use_resource_effect`)

Fires once on wasm mount, never again. Use for pages whose data is stable for the visit (user profile, settings, dashboards that don't need to track external mutations).

```rust
use crate::transport::leptos::signals::use_resource_effect;

#[component]
pub fn PostListPage() -> impl IntoView {
    let items = use_resource_effect(|| load_posts_list(ListQuery::default()));

    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Table>
                <h1>"Post list"</h1>
                {move || match items.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Ok(items)) => render_list_items(&items).into_any(),
                    Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),
                }}
            </PageShell>
        </AuthGuard>
    }
}
```

This is what codegen emits today. Switching a generated page to Polled or Live is a hand-edit (or a future Primer flag — out of scope for the current pass).

### Tier 2 — Polled (`use_polled_resource`)

Fires on mount, then re-fires every `interval_ms` while the document is visible. Pauses on `document.visibilityState == "hidden"` (background tab) and resumes on `visibilitychange`. Returns a `PolledResource<T>` with `signal` + `refetch()` for manual triggers (refresh button, post-mutation reconciliation).

Use when external systems (other users, cron jobs) write to the resource and the page wants stale-after-N-seconds freshness without a WS budget.

```rust
use crate::transport::leptos::signals::use_polled_resource;

#[component]
pub fn FeedPage() -> impl IntoView {
    let feed = use_polled_resource(|| load_feed(), 15_000);
    let on_refresh = move |_| feed.refetch();

    view! {
        <button on:click=on_refresh>"Refresh"</button>
        {move || render_feed(&feed.signal)}
    }
}
```

### Tier 3 — Live (`use_live_resource`)

Fires on mount, opens a single multiplexed Relay WebSocket (`/ws`, cookie-authed via the same-origin upgrade), subscribes to the given topic, and re-fires the loader on **every** inbound frame regardless of payload — the DB is the source of truth, we reconcile from it (see `SPEC_RELAY.md` "Reconnect via DB"). Reconnects on close with exponential backoff (250ms → 8s cap). Closes the socket on owner cleanup.

Use for resources where same-process writes drive the UI (chat threads, presence, live order status).

```rust
use crate::transport::leptos::signals::use_live_resource;

#[component]
pub fn OrderDetailPage() -> impl IntoView {
    let order = use_live_resource(
        || load_order(42),
        "orders:customer:42",
    );

    view! {
        {move || render_order(&order.signal)}
    }
}
```

**Why not `Resource::new` / `LocalResource::new` (binding):**
- `Resource::new` requires `T: Serialize + Deserialize` so SSR-resolved values can travel to wasm via the HTML payload. `MeltDown` carries `Arc<dyn Error>` and is **not** Serialize. Wrapping requires either making MeltDown Serialize (large refactor) or pre-flattening the Result (loses error info).
- `LocalResource::new` is documented as wasm-only but its `.get()` and `Suspense` interop pull `js-sys` statics during SSR rendering. On host that triggers `js-sys-0.3.97 cannot access imported statics on non-wasm targets` → tokio-rt-worker panic → 500 / connection reset. Verified empirically.
- Plain `RwSignal<Option<Result<T, MeltDown>>>` + cfg-gated `Effect::new(spawn_local(load_*))` sidesteps both: SSR renders the Loading placeholder (signal=None), wasm hydrates with the same placeholder (signal=None at first), then Effect fires post-hydrate, fetches via `/api/<r>`, populates the signal, view re-renders. SSR ↔ hydrate render the same tree → no tachys mismatch.

This means **the data is NOT pre-resolved during SSR**. The cold-load HTML payload always shows `<p>"Loading..."</p>` for resource-backed sections. If a page needs server-baked data in the SSR payload, it must use `expect_context::<Ctx>()` in the page body directly (synchronous, since `flows::*::run` returns a future that resolves immediately on SSR with a Pool-bound Ctx). This is the escape valve for hand-written pages that need SEO/perf-critical pre-rendering.

**`api_client.rs`** (canonical, wasm-only) provides `get_json`, `post_json`, `post_unit`, `patch_json`, `delete`. All map BE error envelope (`{error: {melt_type, message}}`) to a `MeltDown` via `parse_or_envelope_error`.

### Parameterized pages (`:id` route param)

Detail/Edit pages live at routes like `/posts/:id` and `/posts/:id/edit`. The page component reads the `id` from the route via `leptos_router::hooks::use_params_map()`, wraps it in a `Memo<i64>`, and the data-fetch `Effect` re-fires whenever the id changes (e.g. user navigates from `/posts/1` to `/posts/2`).

```rust
use leptos::prelude::*;
use leptos::task::spawn_local;
use crate::meltdown::MeltDown;
use crate::structs::generated::posts::PostPublic;
use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell};
// Loader is called only inside a wasm-cfg-gated Effect — keep its import wasm-only.
#[cfg(target_arch = "wasm32")]
use crate::transport::leptos::data::generated::posts::load_posts_one;
// Deleter is called from an unconditional click handler. The data helper itself is
// exported unconditionally with cfg-gated bodies, so the import is unconditional.
use crate::transport::leptos::data::generated::posts::do_posts_delete;

#[component]
pub fn PostDetailPage() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let id_signal: Memo<i64> = Memo::new(move |_| {
        params.read().get("id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(-1)
    });
    let item_signal: RwSignal<Option<Result<PostPublic, MeltDown>>> = RwSignal::new(None);

    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        let id = id_signal.get();      // reactive — re-fires on route change
        if id < 0 { return; }          // skip while route param is missing/unparseable
        item_signal.set(None);          // reset to Loading on every id change
        spawn_local(async move {
            let result = load_posts_one(id).await;
            item_signal.set(Some(result));
        });
    });

    // ... view! ...
}
```

**Sentinel pattern.** `unwrap_or(-1)` returns `-1` when the param is missing or unparseable; the Effect early-returns on `id < 0` so the loader is never called with a bogus id. This is the same shape SSR sees on first render (no params yet → id = -1 → no fetch → `<p>"Loading..."</p>` placeholder rendered), so SSR ↔ hydrate trees agree.

**Effect re-fires on `id_signal` change.** `id_signal.get()` inside the Effect subscribes to `id_signal`. When the user navigates from `/posts/1` to `/posts/2` (via `<A href=...>` or `navigate(...)`), `params` updates, `id_signal` recomputes, the Effect re-runs, `item_signal` clears to `None` (renders Loading), the new fetch resolves, view updates. No router-level boilerplate; everything flows through reactive signals.

**Delete from a Detail page** uses the same `id_signal` (via `get_untracked` inside the click handler — we want the value at click time, not a subscription):

```rust
let on_delete = move |_ev: leptos::ev::MouseEvent| {
    let id = id_signal.get_untracked();
    if id < 0 { return; }
    spawn_local(async move {
        let outcome = do_posts_delete(id).await;
        // ... handle outcome ...
    });
};
```

**Edit pages** follow the same id extraction. The loaded `PostPublic` is passed to `<PostEditForm initial=initial/>`; the form pulls the row's primary key from `initial.<pk_field>` (struct-aware — works for non-`id` PKs) and uses it when calling `do_posts_update(id, patch)`.

## Auth (locked)

httpOnly SameSite=Lax cookie (no Secure flag in dev — Firefox + recent Chrome drop Secure cookies on plain http://localhost). Server reads cookie on SSR request, knows session immediately, can render full page or redirect. Wasm has no JS access (httpOnly). All `/api/*` calls send cookie automatically.

**Wire shape:** `/api/auth/{login,register,me}` all return `SessionContext` (`{session_id, user_id, role}`) directly. `SessionContext.token` carries `#[serde(skip_serializing, default)]` so the cookie token never leaks into the JSON body. The httpOnly cookie is the only place the token lives client-side.

**Auth middleware behavior on stale cookies (binding):** when the request carries a session cookie that no longer resolves (DB row gone, expired, malformed), the middleware MUST swallow the resolve error, log Debug, and fall through to anonymous Ctx. It MUST NOT propagate `SessionInvalid` to the response — doing so 401's `/api/auth/login` itself, making it impossible to recover from a stale cookie. See `src/transport/http/middleware/auth.rs::request_ctx_middleware`.

`AuthGuard` component wraps every protected page. Modes: `Public` (always render), `AnonOnly` (redirect to `/dashboard` when authed — used by `/login`/`/register`), `Required` (redirect to `/login` when anon), `AdminOnly` (redirect to `/login` when anon, `/` when non-admin). Reads from a global `SessionStore` (typed wrapper around `RwSignal<Option<SessionContext>>` at `src/structs/leptos/session_store.rs`).

**Session boot (binding) — both targets agree on `Option<SessionContext>` at first paint:**
- **SSR**: `leptos_routes_with_context` callback reads `axum::http::request::Parts` from leptos context, parses `SESSION_COOKIE`, resolves it via `flows::sessions::resolve` (`block_in_place + block_on` in multi-thread tokio), `provide_context::<Ctx>(ctx_with_session)`. The `shell()` function emits `<script id="cata-session-boot">window.__cata_session = {SessionContext-JSON or null}</script>` in `<head>` from the resolved Ctx. The `provide_session_store()` reads `use_context::<Ctx>()` synchronously and seeds the SessionStore.
- **Wasm hydrate**: `provide_session_store()` reads `window.__cata_session` synchronously via `js_sys::Reflect::get + JSON.stringify + serde_json::from_str` BEFORE the first conditional render. SessionStore is seeded with the same value SSR rendered with. AuthGuard's branch is identical on both targets → no tachys hydration mismatch.
- No `/api/auth/me` round-trip on hydrate. The injected payload is the source of truth at boot. Mutations (`do_login`, `do_register`, `do_logout`) write directly to the SessionStore.

This eliminates the entire class of "tachys: expected a marker node, but found `<main>`" hydration panics that occur when SSR and hydrate disagree on a reactive value driving a conditional render.

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

## Vendored components

User-owned, hand-editable components shipped with every scaffold under `src/transport/leptos/components/`. Two-tier ownership applies — Blast never touches them after scaffold. Fork-by-default.

### Cells (display primitives) — `components/cells/`

Drop-in for `<td>`/`<dd>`/inline value display. Each takes a typed prop and emits a styled element with semantic markup.

| Component | Prop shape | Output |
|-----------|-----------|--------|
| `<DateCell value=DateTime<Utc> format=DateFormat>` | `Iso` / `Short` / `Long` / `Time` | `<time datetime>` |
| `<RelativeDateCell value=DateTime<Utc>>` | "5 minutes ago" / "in 2 hours" | `<time>` |
| `<TimeCell value=DateTime<Utc>>` | HH:mm:ss | `<time>` |
| `<MoneyCell amount=i64 currency=Currency>` | minor units → "$1,234.56" | `<span>` |
| `<NumberCell value=f64 decimals=u8 thousands=bool>` | formatted number | `<span>` |
| `<BoolCell value=bool variant=BoolVariant>` | `Check` / `YesNo` / `Badge` | glyph or pill |
| `<EnumCell<E> value=E color=fn(&E)->&'static str>` | enum variant pill | colored `<span>` |
| `<BadgeCell text=String color=BadgeColor>` | primitive pill | `<span>` |
| `<JsonCell value=serde_json::Value collapsed=bool>` | pretty-printed | `<details><pre>` |
| `<EmptyCell>` | em dash | `<span>—</span>` |
| `<PercentCell value=f64 decimals=u8>` | "N%" | `<span>` |
| `<DurationCell ms=i64>` | "2h 15m" | `<span>` |

Enum types (`DateFormat`, `BoolVariant`, `BadgeColor`, `Currency`) live in `structs/leptos/cells.rs` per `STRUCTS:22`.

### Layout / display — `components/`

| Component | Purpose |
|-----------|---------|
| `<LinkCell to=RouteName text=String>` | Soft-nav `<A>` link styled to token. |
| `<AvatarCell name=String url=Option<String> size=AvatarSize>` | Image fallback to initials, circle wrapper. Size: Sm/Md/Lg. |
| `<StatusDot kind=StatusKind label=String>` | Online/Offline/Pending/Error colored dot + label. |
| `<EmptyState title message action=Option<AnyView>>` | "no data" centered placeholder. |
| `<Skeleton variant=SkeletonVariant>` | Line/Card/Avatar/Button shimmer. |
| `<Pagination total_pages current_page>` | URL-state-bound page nav. Hides if total_pages ≤ 1. |
| `<FilterBar filters=Vec<FilterDef>>` | Debounced filter inputs bound to `use_url_list_state`. |
| `<SortHeader col label>` | Click-to-sort `<th>`, asc/desc/none arrow indicator. |
| `<Breadcrumb items=Vec<BreadcrumbItem>>` | Chevron-separated trail. Last item not linked. |
| `<Tabs items=Vec<TabItem>>` | URL-bound `?tab=name` tab switcher. |
| `<Card title=Option<String>>{children}` | Token-driven card wrapper. |
| `<Stepper steps current>` | Horizontal multi-step indicator. |

### Modals + form widgets — `components/`

| Component | URL-bound? | Purpose |
|-----------|-----------|---------|
| `<ConfirmDialog name title message confirm_label on_confirm>` | yes (`?dialog=<name>`) | Modal overlay with confirm/cancel. |
| `<Drawer name side title>{children}` | yes (`?dialog=<name>`) | Slide-in panel from Left/Right/Top/Bottom. |
| `<Alert kind dismissible>{children}` | no | Info/Success/Warning/Danger banner. |
| `<FormGroup label error>{children}` | no | Label + input slot + error message below. |
| `<FieldError message>` | no | Inline red error under an input. |
| `<HelpText>{children}` | no | Muted hint text. |
| `<InputGroup prefix suffix>{children}` | no | Wraps `<input>` with optional prefix/suffix. |

### Toast helpers — `signals/toast.rs`

Module-level fns: `toast::success(msg)`, `toast::error(msg)`, `toast::info(msg)`, `toast::warning(msg)`. Read from the singleton `ToastStore` provided in `<App>`. `<ToastHost/>` renders the active stack.

### Cells vs render-service builders

The cells/components above are **display primitives** consumed by hand-written pages and codegen. The **render-service builders** in `services/render/` (next section) are higher-order: they take a `Vec<T: Serialize>` or single `T` and produce a complete `<table>`/`<form>`/`<dl>` view. Builders dispatch to cells via formatter closures when fancy rendering is needed.

## Render service (canonical SSR builders)

Runtime SSR component builders at `src/services/render/`. The Leptos port of the rocket-era `services/builders/{table,list,select}.rs` pattern. Drop a `Vec<T>` (or single `T`), get an `impl IntoView` back. Cross-target — compiles on host AND wasm32.

Six builders ship in canonical:

| Builder | Use case |
|---------|----------|
| `TableBuilder<T>` | `Vec<T>` → `<table>` with introspected columns |
| `ListBuilder<T>` | `Vec<T>` → `<ul>` / `<ol>` with introspected items |
| `SelectBuilder<T>` | `Vec<T>` → `<select><option>` for forms |
| `FormBuilder<T>` | `T` → `<form>` with typed inputs (string→text, i64→number, bool→checkbox, etc.) |
| `DetailBuilder<T>` | single `T` → key:value `<dl>` card |
| `StatBuilder<T>` | single `T` → headline number cards in a grid |

### Common pattern

All six follow the same builder API. Generic over `T: Serialize + Clone + Send + Sync + 'static`. Runtime introspection via `serde_json::to_value` — column order = struct field order (serde preserves it). Native HTML output, no third-party table crate.

```rust
use crate::services::render::TableBuilder;

view! {
    <PageShell layout=PageLayout::Table>
        {TableBuilder::new(posts)
            .ignore("id, content, password_hash")
            .class_table("posts")
            .formatter("created_at", |v| view!{ <RelativeDateCell value=parse_dt(v)/> }.into_any())
            .formatter("status", |v| view!{ <BadgeCell text=v.to_string() color=BadgeColor::Success/> }.into_any())
            .empty_text("No posts yet.")
            .into_view()}
    </PageShell>
}
```

### Builder methods (typical)

- `new(items)` — construct.
- `.ignore("col1, col2, col3")` — comma/space-separated columns to skip.
- `.formatter(col, closure)` — per-column override returning `AnyView`. Default: `Display` of the JSON value.
- `.class_<part>(c: &str)` — append a class to `<table>` / `<thead>` / `<tr>` / `<td>` / etc.
- `.empty_text(msg)` — fallback shown when list/record is empty (default "No items.").
- `.into_view()` — consume self, return `AnyView`.

`SelectBuilder` adds `.label_field`/`.value_field` (which struct field becomes option text vs value). `FormBuilder` adds `.label`/`.placeholder`/`.input_kind` overrides + `.on_submit(closure)`. `StatBuilder` requires `.stat(field, label)` calls to declare which fields render.

### When to use which path

| Path | Best for |
|------|----------|
| **Codegen pages** (`<R>ListPage`, `<R>DetailPage`, etc.) | Resources defined in a Primer. Django-admin-style — wired end to end by `blast gen all`. |
| **Render-service builders** | Ad-hoc views over query results. Custom dashboards. Reports. Anywhere the data shape isn't a Primer resource. |
| **Hand-rolled `view!`** | Fully bespoke layouts where neither the codegen nor the builder fits cleanly. |

All three coexist; pick the lightest path that does the job.

### File layout

```
src/services/render/
├── mod.rs
├── table.rs       (+ table.module.scss)
├── form.rs        (+ form.module.scss)
├── list.rs        (+ list.module.scss)
├── select.rs      (+ select.module.scss)
├── detail.rs      (+ detail.module.scss)
└── stat.rs        (+ stat.module.scss)

src/structs/services/render/
├── mod.rs
├── table.rs       (TableBuilder, TableRenderClasses, Formatter)
├── form.rs        (FormBuilder, FieldMeta, FormPlanEntry, InputKind, ...)
├── list.rs        (ListBuilder, ListType, ListItemTemplate)
├── select.rs      (SelectBuilder)
├── detail.rs      (DetailBuilder, DetailFormatter)
└── stat.rs        (StatBuilder, StatField, StatFormatter)
```

`STRUCTS:22` forces struct/enum data definitions into `structs/`; the `services/render/` files carry only `impl` + private helpers.

## Tables (locked)

Codegen for resources at `gen_level >= Components` emits a sibling `<R>TableRow` next to `<R>Public` in `src/structs/generated/<r>.rs`:

```rust
#[derive(Debug, Clone)]
pub struct PostTableRow {
    pub id: i64,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    // ... display-safe Public fields only
}

impl From<PostPublic> for PostTableRow { /* field-by-field move */ }
```

**Why a sibling and not iterating Public directly:** `<R>Public` may legitimately carry types whose Rust mapping has no `Display` impl in our enabled feature set — `serde_json::Value` from `Jsonb`, `Vec<u8>` from `Bytea`, `rust_decimal::Decimal` from `Numeric`. The sibling `<R>TableRow` strips those columns so the row struct can be rendered with `format!("{}", row.field)` cells without the page leaking type errors as soon as the schema introduces a Jsonb/Bytea column. The skip-list for SQL types is in `blast/src/codegen/structs/emitter/table_row.rs::is_display_safe`.

**No third-party table crate.** `leptos-struct-table` v0.14.0-beta2 was the only leptos-0.7-compatible version and is broken upstream (lifetime-bound mismatch with current leptos macro expansion); v0.15+ require leptos 0.8 which is a framework-version bump we are not taking. The native `<For>` + `<table>` approach is straightforward and fits the "FE is a dumb relayer" rule.

**Generated List page renders the table:**

```rust
fn render_list_items(items: ListResponse<PostPublic>) -> impl IntoView {
    let rows: Vec<PostTableRow> = items.items.into_iter().map(PostTableRow::from).collect();
    let has_rows = !rows.is_empty();
    view! {
        <Show when=move || has_rows fallback=|| view! { <p>"No items."</p> }>
            <table>
                <thead>
                    <tr>
                        <th>"id"</th>
                        <th>"title"</th>
                        <th>"created_at"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.clone().into_iter().map(|row| view! {
                        <tr>
                            <td>{format!("{}", row.id)}</td>
                            <td>{format!("{}", row.title)}</td>
                            <td>{format!("{}", row.created_at)}</td>
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </Show>
    }
}
```

Empty state branches via `<Show when=has_rows fallback>` so the `<table>` element is only emitted when there are rows. Loading state stays the wider page-level `Option::None` arm with `<p>"Loading..."</p>`.

## Wasm-only widgets

For hand-rolled components that call into `wasm-bindgen` statics (date pickers, color pickers, anything from `thaw` or `web-sys` that panics at SSR-render time with `js-sys-0.3.97 cannot access imported statics on non-wasm targets`), gate the mount on hydration completion via the global hydration signal.

`src/transport/leptos/signals/hydration.rs` provides:

```rust
pub fn provide_hydration_store() -> RwSignal<bool>;  // wired in <App> alongside session/toast stores
pub fn use_hydration() -> RwSignal<bool>;            // pulled by widgets that need to know
```

SSR seeds the signal at `false` and never flips it. Wasm hydrate seeds the signal at `false` then an `Effect::new` (cfg-gated to wasm32) flips it to `true` once after mount.

Pattern for wasm-only widgets:

```rust
use crate::transport::leptos::signals::hydration::use_hydration;

#[component]
pub fn DatePickerIsland(/* props */) -> impl IntoView {
    let hydrated = use_hydration();
    view! {
        <Show when=move || hydrated.get() fallback=move || view! { <div class="date-picker-skeleton" /> }>
            <thaw::DatePicker /* ... */ />
        </Show>
    }
}
```

SSR renders only the `<div class="date-picker-skeleton" />` — no `thaw::DatePicker` body, so no `js-sys` static access during SSR render. Wasm hydrate renders the skeleton too at first paint (signal still `false`), then the post-mount Effect flips the signal, the `Show` swaps to the children branch, and the real widget mounts. The fallback skeleton is what bridges the SSR ↔ hydrate render-tree match — anything that diverges between `Show`'s `false` arm on SSR and `false` arm on wasm-pre-mount risks tachys hydration mismatch.

This pattern is **only for hand-rolled wasm-only components** that pull `wasm-bindgen` statics during render. Codegen'd forms and pages stick to native HTML and don't need it.

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

Codegen'd forms (`<R>CreateForm`/`<R>EditForm`) follow the same pattern with **native HTML** inputs (`<input>`, `<select>`, `<textarea>`, `<form>`) and `validate_<r>_*` called BEFORE the spawn_local dispatch. **Thaw widgets are banned in codegen** because they panic on SSR (`js-sys-0.3.97 cannot access imported statics on non-wasm targets`). Hand-rolled wasm-only components MAY use thaw, but anything that SSRs must stick to native HTML.

No `#[server]` macro use. Plain axum handlers + Leptos pages, both calling the flow.

## Validators

Single Rust source (`src/structs/generated/validators/<r>.rs`). Compiles to both server binary and wasm because the whole crate compiles to both. Same fn called from REST handlers AND from form components.

See `SPEC_VALIDATORS.md`.

## CSS

`style/main.scss` is the entry; uses `tokens.scss` (design tokens) and `base.scss` (reset + root font scaling). Cargo-leptos compiles via `grass` to `target/site/pkg/canonical.css`.

Per-component `.module.scss` files use `stylance` for hashed-classname scoping. No global styles outside `tokens.scss` / `base.scss`.

OKLCH color, semantic class names, no hex outside theme overrides, no inline styles. Lint rules carry forward into a `LEPTOS:N` build.rs lint family.

## Lint family (LEPTOS:1–10)

Defined in `templates/canonical/build.rs`. Violations panic the compile. No `#[allow]` escape hatch.

| Rule | What it bans |
|------|--------------|
| `LEPTOS:1` | inline `style=` / `style=format!` inside `view!` macros under `transport/leptos/`. Use `.module.scss` + stylance. |
| `LEPTOS:2` | raw color literals (`#hex`, `rgb()`, `hsl()`, etc.) in `transport/leptos/` and `structs/`. Define tokens in `style/tokens.scss`, consume via `var(--app-color-*)`. |
| `LEPTOS:3` | raw `px` in `transport/leptos/`. Allowed exceptions: `0.0625rem` hairline borders, `@media` query breakpoints, anything inside `style/tokens.scss` / `style/base.scss`. |
| `LEPTOS:4` | page components (file under `transport/leptos/pages/`) without `<PageShell layout=...>` wrapping the top-level view. |
| `LEPTOS:5` | hardcoded route paths in `nav("/...")`, `<a href="/...">`, `<A href="/...">`. Use `RouteName::*.path()`. |
| `LEPTOS:6` | optimistic mutation of a list/map signal between `pending.set(true)` and `spawn_local(`. Mutate only after server response. Heuristic — false positives possible. |
| `LEPTOS:7` | `"Loading..."` literal outside `Option::None =>` arms or `Suspense fallback=...`. After first load, refetch silently. |
| `LEPTOS:8` | `ListQuery::default()` outside `signals/url.rs`. List page state lives in the URL via `use_url_list_state`. |
| `LEPTOS:9` | `RwSignal::new(false)` adjacent to a `dialog`/`drawer`/`modal`/`popup`-named identifier. Use `use_query_dialog(name)` for URL-bound modal state. Heuristic — false positives possible. |
| `LEPTOS:10` | `font-size:` declarations on form-control selectors (`input`/`select`/`textarea`/`button`) inside `.module.scss` that don't reference `var(--app-fs-*)` or `inherit`. UA-default control fonts bypass the rem-scaled root and stay tiny at 4K; base.scss pins them globally — per-component overrides must keep the contract. |

`LEPTOS:6` and `LEPTOS:9` are heuristic. If they false-positive on legitimate code, restructure (move the offending pattern to a dedicated function, rename the binding so it doesn't carry a dialog token) rather than disabling the rule.

## WebSocket (Relay)

Server-side `transport/ws/` unchanged from pre-leptos design. Client-side replaces TS `client.ts` with `gloo-net::websocket`. Subscription primitive: `use_topic<T>(topic: &str) -> ReadSignal<Option<T>>` lives in `src/transport/leptos/signals/relay.rs`.

Auth handshake via cookie on WS upgrade (cookie sent automatically by browser on same-origin upgrade). Server reads cookie during upgrade handler, loads session, attaches to relay state.

## Page metadata

Per-page `<Title text="..."/>` via `leptos_meta`. `<MetaTags/>` mounted in `<App>` `<head>` slot. OG tags + favicons via `leptos_meta::Meta` and `Link`.

## Dark mode

Three states: `Light`, `Dark`, `System` (the default). System follows the browser's `prefers-color-scheme` media query; `Light` and `Dark` force the palette regardless of OS preference. Defined as `Theme` enum at `src/structs/leptos/theme.rs`.

**Wire shape (locked):**
- Persistence: `theme=light|dark|system` cookie (`SameSite=Lax`, `Path=/`, `Max-Age=31536000`). NOT httpOnly — wasm needs read/write to mirror signal changes back.
- Boot:
  - **SSR**: `signals/theme.rs::ssr_resolve_theme()` reads `axum::http::request::Parts` from leptos context, parses the `theme` cookie. `app.rs::shell()` emits `<html lang="en" data-theme="light|dark|system">` directly in the served HTML — no flash. `provide_theme_store()` re-resolves from the same `Parts` and seeds `RwSignal<Theme>` in context.
  - **Wasm hydrate**: `provide_theme_store()` reads `document.cookie` synchronously via `web_sys::HtmlDocument::cookie()` BEFORE the first conditional render. Same value SSR rendered with → no hydration mismatch.
- Mirror Effect (wasm only, inside `provide_theme_store`): on every signal change → `document.documentElement.setAttribute("data-theme", value)` AND `document.cookie = "theme=<v>; SameSite=Lax; Path=/; Max-Age=31536000"`. No round-trip; the next SSR will read the freshly-written cookie.

**Toggle component:** `<DarkModeToggle/>` at `src/transport/leptos/components/dark_mode_toggle.rs`. Cycles `Light → Dark → System → Light`. Mounted in the dashboard header by default — re-use anywhere via `use crate::transport::leptos::components::DarkModeToggle`.

**CSS cascade (binding, in `style/tokens.scss`):**
1. `:root { ... }` — light tokens (default, also the "system + OS=light" case).
2. `@media (prefers-color-scheme: dark) { :root { ... } }` — dark tokens for "System + OS=dark".
3. `:root[data-theme="light"] { ... }` — explicit Light wins over OS dark preference.
4. `:root[data-theme="dark"] { ... }` — explicit Dark wins over OS light preference.

Specificity: `:root[data-theme=...]` (0,1,1) beats `:root` and the `@media` `:root` (both 0,0,1). Manual toggle always wins; System falls through to the `@media` block when present.

**No flash, no JS-required boot:** the SSR-emitted `<html data-theme>` is correct on first byte. Tokens are CSS-only — no JS needed to apply the theme. WASM hydration only takes over for live toggles after the user clicks.

## i18n

Out of scope.

## End-to-end testing

Standalone `e2e/` workspace member ships with the canonical template. Drives a headless Firefox via geckodriver + fantoccini (rust webdriver) + cookie-aware reqwest. Boots `cargo leptos serve`, waits for `/api/healthz`, then exercises:

1. `smoke_welcome` — load `/`, install console hook, sanity-check JS thread.
2. `register_via_ui` — fill form, submit, await `/dashboard`.
3. `logout_via_ui` — `goto /logout`, await redirect to `/login`. (NOT `logout_via_api` — fantoccini and reqwest have separate cookie jars; api logout doesn't clear the browser session.)
4. `login_via_ui` — fill form, submit, await `/dashboard`.
5. `cold_dashboard_after_login` — full-page `goto /dashboard`, assert URL stays, `<h1>Dashboard</h1>` present, no fatal console errors. **Catches SSR↔hydrate session mismatch.**
6. `cold_login_redirects_when_authed` — full-page `goto /login` while authed, assert URL flips to `/dashboard`. **Catches AnonOnly regression.**
7. `auth_me_via_api` — reqwest sanity check.
8. `cold_posts_list` — full-page `goto /posts`, assert h1 has "post", no console errors.
9. `cold_posts_create` — full-page `goto /posts/new`, assert `<form.posts-create-form>` rendered, no console errors.

Console-error capture:
- `install_console_hook` injects a JS hook that captures `console.log/warn/error`, `window.onerror`, `unhandledrejection` into `window.__e2e_console`.
- `assert_no_fatal_console` drains and matches against `FATAL_CONSOLE_SUBSTRINGS` (`"hydration"`, `"panicked"`, `"unreachable executed"`, `"Unrecoverable"`, `"expected a marker node"`, `"RuntimeError"`, `"wasm-bindgen"`). Any match → step fails.
- Called at the end of every step. Hydration panics no longer slip past silently.

Wire-up:
- `[package.metadata.leptos] end2end-cmd = "cargo run --release"` + `end2end-dir = "e2e"` in canonical's Cargo.toml.
- `cargo leptos end-to-end` (or `blast e2e`) builds + serves + runs the e2e binary.
- pre-req: `pacman -S geckodriver firefox` (or chromedriver — fantoccini handles either; default in this template is geckodriver since Firefox is the primary dev browser).

The e2e binary spawns its own geckodriver on port 4444 in a subprocess and kills it on exit. It does NOT touch the user's running Firefox — geckodriver creates an isolated profile per session. Headless mode via `moz:firefoxOptions.args = ["-headless"]`.

Diagnostic patterns baked into the e2e (kept in for future debugging):
- `setTimeout(...click(), 50)` instead of `client.click()` so we can poll the JS thread post-submit without WebDriver waiting for page-settled.
- `tokio::time::timeout(2s, client.execute("return Date.now()"))` poll loop — if it doesn't return, the wasm hydrate has wedged the event loop (regression check for the `Action::new_local + Effect::new(action.value())` deadlock pattern).
- fetch hook (`window.fetch = wrapper`) records all outbound XHRs so we can assert the right `/api/*` endpoint was hit even if the resulting navigate happens too fast to observe.
- per-step `tokio::time::timeout` of `STEP_TIMEOUT` (40s) wrapping each phase. Master timeout at 180s to guarantee the suite never hangs the runner.

## Related specs

- `SPEC_ARCHITECTURE.md` — layer rules
- `SPEC_PRIMER.md` — per-resource state file
- `SPEC_VALIDATORS.md` — single-source validator codegen
- `SPEC_FLOWS.md` — flow authoring
- `SPEC_MELTDOWN.md` — error type + wire shape
- `SPEC_RELAY.md` — WS protocol
- `SPEC_SESSIONS.md` — auth tokens (cookie transport)
- `blast/doc/SPEC_CODEGEN.md` — what Blast emits for the leptos UI
