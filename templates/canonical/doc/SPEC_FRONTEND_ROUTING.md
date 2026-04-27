# SPEC_FRONTEND_ROUTING

Routing, navigation, modals, and reactivity tier philosophy for Catablast frontend. Locks the discipline that keeps a Vue SPA behaving like a respectable multi-page app — every URL real, refresh-survivable, back-button correct.

## Why this spec exists

SPAs broke the web's URL contract. `/users/42` used to mean "this thing, deep-linkable, refreshable, shareable, browser-history-correct." Modern SPA practice silently moved state into local refs, modals, optimistic mutations — none of which survive a refresh, share, or back button. Catablast rejects that drift.

Catablast cannot ship server-side rendering (locked stack: single Rust binary, no Node runtime), so it cannot truly serve `/file.html` per-URL. What it CAN do: enforce SPA-as-disciplined-MPA via codegen + Governor lints. The result feels like the old web, runs on Vue.

## The two URL axes

**Path = identity.** What resource am I looking at? Hierarchy of nouns.

| Path | Meaning |
|------|---------|
| `/users` | Collection (list view) |
| `/users/42` | Instance (detail view) |
| `/users/new` | Verb on collection (create page) |
| `/users/42/edit` | Verb on instance (edit page) |
| `/users/42/posts` | Sub-collection of related resources |
| `/users/42/posts/7` | Nested instance |

Heuristic: if changing the value puts you on **different content** (different noun, different data fetch), it's a path segment.

**Query = view state on same content.** How am I looking at it?

| Query | Meaning |
|-------|---------|
| `?page=2&page_size=25&sort=-created_at&filter[status]=active` | Paginated list view state. Wire contract — see `SPEC_FRONTEND.md`. |
| `?q=searchterm` | Search input on a list. |
| `?dialog=user-edit&dialog_id=42` | Modal overlay state (see Modals below). |
| `?tab=settings` | Tab selection ONLY when tabs are pure UI affordance over the same data fetch. Otherwise promote to path. |

**Heuristic for the gray zone:** if you can imagine bookmarking the URL and expecting to land on the *same thing* with the *same data scope* and *same UI affordances open*, then the state goes in the URL. No hidden state.

## Routes are codegen'd from a single source

Both routes and navigation come out of `storage/blast/state/app.ron` (Blueprint) + `storage/blast/state/resources/<name>.ron` (Primer). One codegen pass produces:

| Output | Purpose |
|--------|---------|
| `frontend/src/generated/router/routes.ts` | vue-router config: paths, components, full meta (label, icon, section, roles, breadcrumb). |
| `frontend/src/generated/router/route-names.ts` | String union type. `{ name: 'users.list' }` becomes compiler-checked. |
| `frontend/src/generated/router/install-router-guards.ts` | Auth + role gating from route meta. Single guard install point. |
| `frontend/src/generated/nav/menu.ts` | Typed menu tree consumed by AppSidebar / AppBreadcrumb / AppTopbar. |

**Drift is impossible by construction.** A Blueprint nav `Entry(route: "users.list", ...)` whose route doesn't exist → codegen fails. A nav role mismatch with the route's auth meta → codegen fails. Hand-maintained `ROUTE_TO_KEY` mappings (see fresh project's pain) DO NOT EXIST in Catablast.

### Resource CRUD route auto-emission

For every Primer resource with verbs enabled, Blast emits a default route set:

| Verb | Route name | Path | Layout |
|------|-----------|------|--------|
| `List` | `users.list` | `/users` | `table` |
| `Get` | `users.detail` | `/users/:id` | `cards` |
| `Create` | `users.create` | `/users/new` | `cards` |
| `Update` | `users.edit` | `/users/:id/edit` | `cards` |

Resource routes auto-appear in nav under the `resources` section unless overridden by Blueprint.

### Custom (non-CRUD) routes

Declared in Blueprint `pages` section:

```ron
pages: [
    Page(
        route: "dashboard",
        path: "/",
        component: "DashboardPage.vue",
        layout: "cards",
        label: "Dashboard",
        icon: "dashboard",
        in_nav: true,
        roles: [User, Admin],
    ),
    Page(
        route: "debug.thing",
        path: "/_debug/thing",
        component: "DebugThing.vue",
        layout: "bleed",
        in_nav: false,
    ),
]
```

The `component:` field references a hand-written Vue file in `src/` (user-owned, any path outside `generated/`) — codegen wires the route to it but does not generate the page body itself.

### Nav declaration

```ron
nav: NavConfig(
    sections: [
        Section(
            key: "main",
            label: "Main",
            icon: "home",
            entries: [
                Entry(route: "dashboard"),
                Entry(route: "users.list", roles: [Admin]),
                Entry(route: "orders.list"),
            ],
        ),
        Section(
            key: "ops",
            label: "Operations",
            icon: "tools",
            roles: [Admin],
            entries: [
                Entry(route: "fuses.list"),
                Entry(route: "audit.list"),
            ],
        ),
    ],
)
```

Sections render as menu groups in AppSidebar. Role-gated sections hide entirely for unprivileged users; role-gated entries hide individually. The menu component is consumer-of-codegen, not codegen itself — devs can swap AppSidebar.vue for AppTopbar.vue, both consume `frontend/src/generated/nav/menu.ts` typed tree.

## History mode + Axum SPA fallback

Vue Router runs in **history mode**: `/users/42` not `/#/users/42`. Hash routing is a 2014 hack incompatible with the "URL = real path" mental model.

Axum-side: any request matching `/api/...` or `/ws` routes to handlers. Anything else falls through to a static fallback that serves `frontend/dist/index.html`. That single hand-off is wired in `transport/http/bootstrap.rs` — one route registration, codegen'd into the bootstrap by Blast.

Trailing slash policy: **no trailing slash.** `/users/42`, not `/users/42/`. vue-router default; matches the Unix file-path mental model the user wants.

## Blocking navigation with global progress

Navigation is **blocking**. URL does NOT change until the destination route's data is fetched. Current page stays on screen during the transition. A global progress bar (top viewport, GitHub-style) indicates loading.

```
User clicks <router-link :to="{ name: 'users.detail', params: { id: 42 } }">
  ↓
beforeResolve guard fires
  ↓
Global progress bar starts (0% → indeterminate)
  ↓
Route component's <Suspense> async setup() runs:
  data fetch via codegen'd useUser(42)
  ↓
Data ready (or 500ms budget exceeded)
  ↓
URL updates to /users/42
Page swaps in fully populated (or with budget-exceeded fallback)
Global progress bar fills + fades
```

### 500ms budget

If data isn't back in 500ms (configurable per-route, default global), navigate anyway. The destination component renders with a small per-section "still loading" indicator on parts that are still pending. `Crank` (retry combinator) handles the timeout enforcement and abort signaling.

This is the compromise between "instant nav with skeletons everywhere" (modern SPA, philosophy violation) and "page hangs forever on slow query" (pre-AJAX MPA pain). 500ms = perceived-instant for fast queries, "something's loading" perception for slow ones.

### Cancellation

Click a second link mid-nav: in-flight `AbortController` aborts the pending fetch, new nav starts. Codegen'd composables wire `AbortSignal` through every fetch.

### Initial cold load

`index.html` ships a minimal app shell (logo + visible global progress bar) from t=0. Vue mounts, fetches initial route data via blocking nav, renders. Same model — no special-cased first-load behavior.

## Modals are URL state

Every modal/dialog/drawer/sidebar overlay reflects in the URL. No exceptions.

```ts

const showEdit = ref(false)
function openEdit() { showEdit.value = true }

<Dialog v-model:visible="showEdit">


const dialog = useQueryDialog('user-edit', { id: 42 })

<Dialog v-model:visible="dialog.visible">
```

`useQueryDialog(name, params?)` is a codegen'd composable that:
1. Reads `?dialog=<name>&dialog_id=<...>` from the route query.
2. Exposes `visible: ComputedRef<boolean>` (true when query matches).
3. Exposes `open(params?)` and `close()` that mutate the query via `router.push({ query: ... })`.
4. Defaults `history: 'push'` (modal toggle adds a history entry; back button closes modal). Override with `history: 'replace'` for transient overlays.

**Refresh-survival**: yes. **Share-link-restores-modal**: yes. **Back button closes modal**: yes. **Forward reopens**: yes. All free.

`useQueryDrawer(name)` and `useQueryPopover(name)` follow the same pattern.

### Exception: ephemeral non-state overlays

Tooltips, hover menus, transient toasts — these are not "state" in the bookmarkable sense. Local refs are fine for these. Governor's `LocalModalState` rule whitelists by component pattern (heuristic: PrimeVue `<Tooltip>`, `<Menu>` triggered by hover, `<Toast>`).

## Reactivity tiers (composable options)

State always lives on the server. Composables fetch from server, expose reactive refs, refetch on triggers. The "tier" is just **what triggers the refetch**.

All resource composables share one shape:

```ts
const { data, error, refetch, page, sort, filter } = useUsersList(opts)
```

Triggers configured per-call:

```ts

useUsersList()


useUsersList({ poll: 5000 })


useUsersList({ live: true })


useUsersList({ poll: 30000, live: true })
```

| Tier | Trigger | Use case |
|------|---------|----------|
| Static | `refetch()` only | Settings page, profile edit, audit log detail. Action buttons call `refetch()` after mutation. |
| Polled | `setInterval(refetch, ms)` | Slowly-changing dashboards, near-real-time lists where WS overkill. Pauses on `document.visibilityState === 'hidden'`. |
| Live | WS subscribe to topic | Chat lists, live operations, anything where stale > 1s is unacceptable. WS event = "this changed, refetch this." Payload-light, server-of-truth. |

### No optimistic UI in custom

Action handlers go: action → pending state → await server → reconcile from response. Codegen'd action helpers do this:

```ts
const updateUser = useUpdateUser()
const { error } = await updateUser(42, { email: 'new@x.com' })

```

Custom code that mutates local state pre-server is flagged by `OptimisticUpdateInCustom` Governor rule.

### No loading spinners after first load

Once a composable's `data` ref is populated, refetch happens silently in background. Stale-while-revalidate. No skeleton flicker. The blocking nav model means the FIRST load already happened with the global progress bar visible — subsequent refetches don't need their own spinner.

Async actions (file uploads, long-running operations) are an exception — those get explicit per-action progress UI.

## Page shell layout enum

Every page wraps content in `<PageShell layout="...">`. Layout is enum-locked, no inline padding/margin/gap props. See `SPEC_FRONTEND.md` for the layout catalog (`cards`, `split`, `table`, `bleed`, `tabbed`).

Route meta carries the chosen layout. Codegen emits `<PageShell :layout="route.meta.layout">` wrapper into auto-generated CRUD pages. Custom pages declare layout in Blueprint and the codegen'd router meta carries it through.

## Forbidden patterns (Governor-enforced)

| Rule | Bans |
|------|------|
| `HardcodedRoutePath` | `router.push('/users')`, `<router-link to="/orders">`. Force named routes. |
| `LocalModalState` | `ref(false)` for `v-model:visible`. Force `useQueryDialog`. |
| `LocalListState` | Local `ref` for page/sort/filter on list views. Force composable URL-bound state. |
| `OptimisticUpdateInCustom` | Pre-server local mutation in custom action handlers. |
| `LoadingSpinnerAfterFirstLoad` | `v-if="loading"` after initial data populated. |
| `PageShellRequired` | Page components without `<PageShell>` root. |

## Related specs

- `SPEC_FRONTEND.md` — Vue/TS/Vite, layout enum, page shell, list wire contract, composable shape.
- `SPEC_RELAY.md` — WS multiplexer, topic subscription protocol used by Tier 3 composables.
- `SPEC_CSS.md` — token system that page layouts and modal overlays consume.
- `blast/doc/SPEC_GOVERNOR.md` — full rule list and enforcement.
- `blast/doc/SPEC_STATE.md` — Blueprint `nav` and `pages` sections that drive route + menu codegen.
- `blast/doc/SPEC_CODEGEN.md` — what Blast emits for routing/nav.
