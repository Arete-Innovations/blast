# SPEC_FRONTEND

Vue 3 + TypeScript + Vite + PrimeVue. Packaged into the Rust binary and served by Axum.

## Stack

| Concern | Choice |
|---------|--------|
| Framework | Vue 3 (`<script setup lang="ts">`) |
| Language | TypeScript (required) |
| Build tool | Vite |
| UI library | PrimeVue (Lara/Aura preset, remapped to app tokens) |
| Router | Vue Router (history mode; see `SPEC_FRONTEND_ROUTING.md`) |
| State | Codegen'd composables per resource (per-resource scope) + URL params (view state) + singleton bus (cross-resource events). **No Pinia.** |
| Styling | Monolithic `tokens.css` + scoped `<style>` in SFCs (see `SPEC_CSS.md`) |
| HTTP | Generated typed API clients (fetch-based) |
| WebSocket | Shared `WsClient` singleton backed by `Relay` protocol (see `SPEC_RELAY.md`) |
| Page layout | Enum-locked: `cards` / `split` / `table` / `bleed` / `tabbed` (see Layouts below) |
| Modals | URL state via `useQueryDialog`/`useQueryDrawer` (see `SPEC_FRONTEND_ROUTING.md`) |
| Navigation | Blocking with 500ms budget + global progress bar |

NOT used:
- Pinia (banned by `PiniaImport` Governor rule)
- Tailwind / utility-class CSS
- React
- HTMX as primary path (allowed as tactical choice but not default)
- Nuxt / SSR (single-binary deployment lock)
- Materialize
- Local component state for view-affecting things (page, sort, filter, modal visibility) — all of those are URL state

## Directory Layout

```
frontend/
├── package.json
├── vite.config.ts
├── scripts/
│   └── governor-plugin.js              (Vite plugin: invokes `blast check` on prebuild/predev)
├── src/
│   ├── main.ts
│   ├── App.vue                         (mounts <RouterView/>, global progress bar, app shell)
│   ├── styles/
│   │   ├── tokens.css                  (design tokens — monolithic, single source of truth)
│   │   └── base.css                    (reset + root font-size scaling)
│   ├── plugins/
│   │   └── primevue.ts                 (ONLY file allowed to have hex colors; lint-exempt)
│   ├── icons.ts                        (icon class registry, lint-exempt)
│   ├── generated/                      (Blast emits; regenerable; hash-locked)
│   │   ├── types/
│   │   │   ├── users.ts
│   │   │   ├── orders.ts
│   │   │   └── meltdown.ts             (MeltType enum mirror)
│   │   ├── api/
│   │   │   ├── users.ts
│   │   │   └── orders.ts
│   │   ├── composables/
│   │   │   ├── users.ts                (useUsersList, useUser, useCreateUser, ...)
│   │   │   ├── orders.ts
│   │   │   └── url.ts                  (useQueryDialog, useQueryDrawer, useQueryParam, useUrlListState)
│   │   ├── router/
│   │   │   ├── routes.ts               (vue-router config, full meta)
│   │   │   ├── route-names.ts          (string union type for compiler-checked names)
│   │   │   └── install-router-guards.ts (auth + role gating)
│   │   ├── nav/
│   │   │   └── menu.ts                 (typed menu tree consumed by AppSidebar etc.)
│   │   ├── ws/
│   │   │   └── client.ts               (singleton WsClient via Relay protocol)
│   │   ├── bus.ts                      (cross-resource event bus, singleton)
│   │   └── components/                 (PageShell, layouts, generated CRUD Form/List per resource)
│   ├── pages/                          (auto-emitted CRUD pages: <Resource>ListPage.vue, etc.)
│   ├── DashboardPage.vue               (user-authored pages live alongside generated; Blast never touches)
│   ├── SettingsPage.vue
│   └── components/                     (user-authored components and composables)
└── dist/                               (Vite output — served by Axum, gitignored)
```

Both `generated/` and all user-authored files in `src/` are linted by Governor (`SPEC_GOVERNOR.md`). Generated should be compliant by construction; linting it is a forcing function on the codegen.

## Packaging

Frontend is a sibling directory of the Rust app. Vite builds to `frontend/dist/`. Rust `bootstrap.rs` mounts `dist/` as a static-file service in Axum, with SPA fallback (any non-API path serves `index.html` so vue-router history mode works).

Build pipeline:
1. `blast check` — lints FE source via `Governor` (see `blast/doc/SPEC_GOVERNOR.md`)
2. `vite build` — outputs to `dist/`
3. `cargo build --release` — embeds via `include_dir!` or serves from disk depending on `BUILD_MODE`

Dev mode: `blast run` starts backend + Vite dev server; Axum proxies unmatched routes to Vite's HMR server.

## Page Shell + Layout Enum

Every page wraps its content in `<PageShell layout="...">`. Layout is enum-locked. `PageShell` does not accept `padding`/`margin`/`gap`/`width`/`height` props — pick a layout, layout owns the spacing.

| Layout | Padding | Use case |
|--------|---------|----------|
| `cards` | `--app-space-md` all around + gap between cards | Default. Forms, dashboards, anything with stacked card sections. Auto-emitted for CRUD detail/edit/create pages. |
| `split` | Zero left (rail attaches to sidebar/master column), `--app-space-md` right | Master-detail: list on left, detail on right. List composables drive both panes. |
| `table` | Zero padding, full viewport height, scroll inside the table | Full-bleed data tables / list views with own toolbar + pagination. Auto-emitted for CRUD list pages. |
| `bleed` | Zero everything, full viewport, no chrome | Maps, canvases, full-screen viz, landing pages. Component owns its own padding. |
| `tabbed` | Zero top, child tab content picks its own layout | Tab container; each tab can declare its own nested layout. Tab routes live as path segments (`/users/42/profile`, `/users/42/posts`), not query params. |

```vue

<script setup lang="ts">
import { useDashboardStats } from '@/generated/composables/dashboard';

const { data } = useDashboardStats();
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <PageTitle title="Dashboard" />
    </template>
    <DashboardCard v-for="card in data ?? []" :key="card.id" :card="card" />
  </PageShell>
</template>
```

`<PageShell>` exposes `<template #header>` slot for page title + actions. Layout positions the header consistently. CRUD page codegen fills the header from Primer `display_name` + `verbs`.

Responsive behavior locked at layout level (e.g. `split` collapses to stacked on narrow viewport, `table` toolbar wraps). Pages don't override responsive behavior; they pick a layout that fits.

## Three Reactivity Tiers (composable options)

All resource composables share one shape; the tier is what auto-triggers refetch. See `SPEC_FRONTEND_ROUTING.md` for full reactivity contract.

```ts
const { data, error, refetch, page, sort, filter } = useUsersList(opts)
```

| Tier | `opts` | Trigger |
|------|--------|---------|
| Static | `{}` | Mount + manual `refetch()` only |
| Polled | `{ poll: 5000 }` | Mount + `setInterval`; pauses on `document.visibilityState === 'hidden'` |
| Live | `{ live: true }` | Mount + WS subscription via Relay; payload = "row N changed", composable refetches from server |

Pagination/sort/filter come from URL query params via `useUrlListState` baked into the composable. Local refs for these are banned (`LocalListState` Governor rule).

## List Endpoint Wire Schema

All generated list endpoints use a fixed query-param format. Non-negotiable; not configurable per-resource.

### Format

```
GET /api/orders?page=1&page_size=25&sort=-created_at&filter[status]=open&filter[customer_id]=42
```

| Parameter | Type | Default | Notes |
|-----------|------|---------|-------|
| `page` | integer ≥ 1 | `1` | 1-indexed |
| `page_size` | integer 1–200 | `25` | Hard max: 200. Requests above 200 are clamped, not rejected. |
| `sort` | `±col` | `+id` | `-col` = descending, `+col` or bare `col` = ascending. Single column only. |
| `filter[col]` | string | (none) | Repeated for multiple filters. Only columns declared in `filtered_by` in the resource state file are accepted; unknown filter keys return `400 Bad Request`. |

### Response envelope

```json
{
    "data": [...],
    "meta": {
        "page": 1,
        "page_size": 25,
        "total": 138,
        "total_pages": 6
    }
}
```

`total` and `total_pages` come from a `COUNT(*)` on the same filtered query. Always present on list responses.

### Validation pipeline

Resource state file `filtered_by: ["status", "customer_id"]` → Blast emits:
- `UserListFilters` struct in Rust (Serde-deserialized from query params; unknown keys rejected at deserialization)
- `UserListFilters` TypeScript type (strongly typed, no `Record<string, unknown>`)
- Rust validator in the generated route handler (validates page, page_size bounds, sort column whitelist)
- TS validator in `frontend/src/generated/api/users.ts` (validates before the fetch call; early client-side error for bad inputs)

The TS validator is codegen'd from the resource state file by Blast's `blast gen all` pass. User does not write it. Edit the resource state file via `blast gen resource` and run `blast gen all` to update both validators in sync.

### Sort column whitelist

Only columns declared in the `Admin` or `Public` variant of the resource state file are sortable. Attempting to sort by an unlisted column returns `400 Bad Request`. Blast emits the whitelist as a constant in the generated route handler.

### Anti-pattern

```ts

const url = `/api/orders?page=${page}&sort=${sortField}`;
```

Use `@/generated/api/orders.ts`: `listOrders({ page, page_size, sort, filter })`. It constructs the URL, applies the TS validator, and returns a typed result.

## Generated Composable Shape

For resource state file `orders` with `list`/`get`/`update`/`delete` verbs and WS on `status`:

```ts


export function useOrdersList(opts?: UseListOpts<OrderListFilters>) {
    const data = ref<OrderPublic[] | null>(null);
    const error = ref<MeltDownResponse | null>(null);
    const { page, sort, filter } = useUrlListState();

    return { data, error, refetch, page, sort, filter };
}

export function useOrder(id: Ref<number>) {  }

export function useUpdateOrder() {
    return async (id: number, patch: OrderPatch) => {


    };
}

export function useDeleteOrder() {  }
```

User doesn't hand-write these. Edit the resource state file via `blast gen resource`, run `blast gen all`, composables regenerate.

## Hand-Written Code

Pages, components, hand-written composables, and custom WS handlers live alongside generated code in `frontend/src/` (any path outside `generated/`) and consume the generated composables.

```vue

<script setup lang="ts">
import { useOrdersList } from '@/generated/composables/orders';
import OrderCard from '@/components/OrderCard.vue';

const { data: orders, error } = useOrdersList({ live: true });
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <PageTitle title="Orders" />
    </template>
    <OrderCard v-for="o in orders ?? []" :key="o.id" :order="o" />
    <ErrorBanner v-if="error" :error="error" />
  </PageShell>
</template>

<style scoped>
@layer app {
  .orders-grid {
    display: grid;
    gap: var(--app-space-md);
    grid-template-columns: repeat(auto-fill, minmax(20rem, 1fr));
  }
}
</style>
```

All values from design tokens. No hex, no px, no Tailwind, no inline styles, no Pinia. Lint enforces all of this.

## PrimeVue Preset

```ts

import Aura from '@primeuix/themes/aura';
import { definePreset } from '@primeuix/themes';

export const PRESET_SEMANTIC = definePreset(Aura, {
    semantic: {
        primary: { 500: '#7c3aed',  },

    },
    colorScheme: {
        light: {  },
        dark:  {  },
    },
});
```

PrimeVue tokens are remapped to the `--app-*` token namespace in `tokens.css`. Result: PrimeVue components and hand-written SFCs use the same visual language; swapping the preset swaps the whole theme.

CSS layer order: `reset, primevue, app` — app styles always win.

### PrimeVue: do not reinvent

`PrimeVueReinvented` Governor rule bans wrapping/replacing PrimeVue primitives (`Button`, `Card`, `Dialog`, `Drawer`, `Dropdown`, `DataTable`, etc.) with custom components of the same name. Use PrimeVue directly. If you wrap, name after the domain (`OrderActionsMenu`, not `Menu`).

## MeltDown Error Consumption

```ts
import { MeltType } from '@/generated/types/meltdown';

const { error } = await api.orders.create(input);
if (error?.error.type === MeltType.UniqueViolation) {

}
```

Typed on both sides. No stringly-typed error matching.

## Build-Time Guards

`Governor` (run via `blast check` on prebuild) enforces (full list in `blast/doc/SPEC_GOVERNOR.md`):

**Color/sizing:** no inline hex/rgb/hsl, no `px` outside niches, no raw `rem` outside tokens, no inline `style="..."` or `:style="..."`, icon classes confined to `src/icons.ts`.

**TS hygiene:** no `: any`, no `@ts-ignore`, no silent fallbacks (`|| {}`, `?? []`), no `console.log`, snake_case interface fields.

**Architecture:** no raw `fetch()` outside `generated/api/`, no `new WebSocket()` outside Relay client, no `localStorage` outside persistence layer, no Pinia imports, no PrimeVue primitive reinvention.

**Routing/URL:** no hardcoded route paths (force named routes), no local modal state (force `useQueryDialog`), no local list state (force URL params), no optimistic updates in custom code.

**Layout:** every page roots with `<PageShell layout="...">`, no inline padding/margin/gap on shell, no loading spinners after first load.

**Complexity:** SFC LOC limit, function LOC limit, template depth limit, template LOC limit.

## Anti-Patterns

**FE owning canonical state:**
```ts

const orders = ref<Order[]>([]);
orders.value.push(newOrder);
```

Always use generated composables. They reconcile from DB.

**Partial WS payloads for state diffs:**
```ts

ws.on('order.deleted', ({ id }) => {
    orders.value = orders.value.filter(o => o.id !== id);
});
```

Generated composables handle this via re-fetch or full-row replacement. Don't hand-wire partial diffs.

**Hand-coding API clients:**
```ts

async function fetchUsers() {
    const r = await fetch('/api/users');
    return r.json();
}
```

Use `@/generated/api/users.ts`. Types match the resource state file.

**Hand-rolling regex / validators in pages:**
```ts
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const email_valid = computed(() => EMAIL_RE.test(email.value))
```

Banned. Field-level validation lives in the Primer (`FieldState.validators`) and codegen emits paired Rust + TS validators with byte-identical regex strings — see `SPEC_VALIDATORS.md`. Pages and forms consume `validate<R>Insertable` / `validate<R>Patch` from `@/generated/validators/<r>`. Hand-written auth pages (Login, Register) use the one-off helpers at `@/composables/auth_validators.ts` which mirror the codegen emit shape so the regex string is identical to a `ValidatorRule::Email` rule.

**Inline styles, hardcoded paths, local modal state, Pinia, optimistic updates** — all banned. See Governor rules.

## Related Specs

- `SPEC_FRONTEND_ROUTING.md` — routing, modals as URL state, blocking nav, reactivity tier philosophy
- `SPEC_CSS.md` — design token system, scoped styles
- `SPEC_RELAY.md` — WS protocol, shared WsClient
- `SPEC_MELTDOWN.md` — error type + TS enum mirror
- `SPEC_VALIDATORS.md` — field-level validators codegen pass (rule set, wire-in pattern, regex compatibility)
- `SPEC_CONFIG.md` — resource state files that drive FE codegen
- `blast/doc/SPEC_GOVERNOR.md` — FE lint enforcement (full rule list)
- `blast/doc/SPEC_CODEGEN.md` — what Blast emits for FE
- `blast/doc/SPEC_STATE.md` — Blueprint nav + pages section schema
