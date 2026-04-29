# SPEC_GOVERNOR

Frontend lint engine. Lives in `blast::governor`. Invoked as `blast check`. Written in Rust; wrapped as a Vite plugin for `prebuild`/`predev` hooks.

## Why Governor

Engine governors limit RPM to prevent over-revving. Catablast's Governor limits FE code to the rules that keep the design system, type safety, routing coherence, and responsive scaling intact. Violations fail the build.

Catablast does NOT use a JS-based linter (`.mjs` script). Rule engine is in Rust, inside Blast itself. Reasons:
- Single binary, single language
- Rust regex outpaces Node regex loops on large file sets
- Rules change via `app.ron`, not by hand-editing emitted JS
- Debugging rule logic = read Rust, not obscure JS

## Scope

Governor scans **both `generated/` and `custom/` subtrees**. Rationale:
- `custom/` = AI-written or hand-written user code = where every rule MUST bite.
- `generated/` = Blast-emitted = should already be compliant by construction. Linting it is a forcing function on the codegen and a tripwire for codegen bugs.

Three exempt files (token source-of-truth + PrimeVue preset + base CSS) are listed in `app.ron` and skipped per-rule. Whitelist file (`.rule_violations_whitelist`) carries narrow per-pattern exceptions for hand-written escape hatches.

## Invocation

```
blast check                         # lints frontend/ source tree (custom + generated)
blast check --verbose               # extra diagnostic output
blast check --fix                   # v2; not implemented in v1
```

Exit code 0 = clean. Exit code 1 = violations found (and printed). Blast emits a Vite plugin (`frontend/scripts/governor-plugin.js`) that calls `blast check` on Vite's `prebuild` and `predev` hooks — build fails cleanly if lint fails.

## Rule Config (from `app.ron`)

Rules are not hardcoded in Blast. They're configured via the `fe_lint` section of `storage/blast/state/app.ron`:

```ron
fe_lint: FeLintConfig(
    max_lines_per_sfc: 600,
    max_lines_per_fn: 120,
    max_template_depth: 5,
    max_template_loc: 200,
    hairline_border_rem: "0.0625rem",
    exempt_color_files: ["src/generated/plugins/primevue.ts"],
    exempt_px_files: [
        "src/generated/plugins/primevue.ts",
        "src/generated/styles/tokens.css",
        "src/styles/base.css",
    ],
    whitelist_snippets: ["schema.org"],
    deny_rules: [
        // Color / sizing discipline (apply to global CSS, scoped <style>, inline directives)
        RawColorOutsidePreset,
        HardcodedPx,
        RawRemOutsideTokens,
        InlineStyle,
        IconClassOutsideIconsFile,

        // TS hygiene
        TypeAny,
        TsIgnore,
        SilentFallback,
        ConsoleLog,
        SnakeCaseInterfaceFields,

        // Architectural layering
        RawFetchOutsideApi,
        WebSocketOutsideRelay,
        LocalStorageOutsidePersistence,
        PrimeVueConfigImportOutsidePresetFile,
        PiniaImport,
        PrimeVueReinvented,

        // Routing + URL discipline
        HardcodedRoutePath,
        LocalModalState,
        LocalListState,
        OptimisticUpdateInCustom,

        // Layout / page structure
        PageShellRequired,
        InlineLayoutProps,
        LoadingSpinnerAfterFirstLoad,

        // Template complexity
        MaxLinesPerSfc,
        MaxLinesPerFn,
        MaxTemplateDepth,
        MaxTemplateLoc,
    ],
),
```

Blast reads `storage/blast/state/app.ron` directly and applies the configured rules. The default `deny_rules` list above is the recommended set — opt-out by removing entries, opt-in by adding them.

## Rules

### Color / sizing discipline

**`RawColorOutsidePreset`** — Bans `#[0-9a-fA-F]{3,8}`, `rgb(`, `rgba(`, `hsl(`, `hsla(`, named CSS colors (`red`, `blue`, etc.) outside files in `.exempt_color_files`. Applies in **global CSS, scoped `<style>` blocks, `style=` attributes, `:style` directives, TS string literals assigned to style properties**. The token system (PrimeVue preset → `var(--p-*)`, app tokens → `var(--app-*)`) is the only legal color source.

Default exempt: `src/generated/plugins/primevue.ts`.

**`HardcodedPx`** — Bans `\d+(\.\d+)?px` outside files in `.exempt_px_files`. Applies in **global CSS, scoped `<style>` blocks, inline styles, TS literals**. Line-level exceptions: `@media` queries, `rootMargin:` (IntersectionObserver), explicit hairline-border allow (`0.0625rem` is the rem-equivalent).

Rationale: `px` doesn't scale with root font size; breaks responsive scaling on 4K / high-DPI.

Default exempt files: `src/generated/plugins/primevue.ts`, `src/generated/styles/tokens.css`, `src/styles/base.css`.

**`RawRemOutsideTokens`** — Bans literal `\d+(\.\d+)?rem` in files other than `tokens.css`. Forces `var(--app-*)` token use in components and **scoped styles**.

Line-level exceptions: `grid-template-columns: minmax(...)`, `@media` queries, `letter-spacing:`, `filter: blur(...)`, `backdrop-filter:`, `background-size:`, `box-shadow:` — cases where bare rem has meaning that tokens can't express cleanly.

**`InlineStyle`** — Bans `style="..."` attributes AND `:style="..."` directives on HTML/Vue templates. No exceptions in v1.

Rationale: style belongs in `<style scoped>` blocks bound by class. Inline is the gateway drug to color/px sprinkling.

**`IconClassOutsideIconsFile`** — Icon class names (`pi pi-user`, `ph ph-check`, `fa fa-bell`, etc.) may only appear in `src/generated/icons.ts`. Components consume `IC.user`, `IC.check` from the registry. Class name patterns configured via `.icon_class_patterns([...])` on `FeLintConfig`.

### TS hygiene

**`TypeAny`** — Bans `: any`, `as any`, `<any>` annotations. Escape hatch: `// @allow-any` comment on the line (discouraged).

**`TsIgnore`** — Bans `@ts-ignore` and `@ts-nocheck` comments. Force proper typing.

**`SilentFallback`** — Bans literal-default fallbacks: `something || 'string'`, `?? []`, `?? {}`, `?? 0`, `?? false`. The empty/zero default silently swallows nil and masks bugs. Force explicit branching.

Heuristic is regex-based; line-level `// @allow-fallback` comment escape.

**`ConsoleLog`** — Bans `console.log`, `console.warn`, `console.error`, `console.info`, `console.debug` in committed source. Use `import.meta.env.DEV && console.log(...)` or a dev-only log utility.

**`SnakeCaseInterfaceFields`** — TypeScript interface/type declarations modeling backend resources MUST use snake_case fields (matches Rust struct serialization). Banned: `camelCase` field names in any interface under `frontend/src/**/types/`. Rationale: zero serde-rename drift between Rust and TS.

Codegen'd interfaces are compliant by construction; rule catches user-authored types that drift.

### Architectural layering

**`RawFetchOutsideApi`** — Bans `fetch(`, `axios.`, `XMLHttpRequest` outside `frontend/src/generated/api/`. Custom code MUST import the codegen'd typed client. No hand-rolled HTTP.

**`WebSocketOutsideRelay`** — Bans `new WebSocket(`, `socket.io`, raw WS construction outside `frontend/src/generated/ws/client.ts`. Custom code uses `useTopic()`/`useChannel()` composables that go through the singleton `Relay` client.

**`LocalStorageOutsidePersistence`** — Bans `localStorage.`, `sessionStorage.`, `indexedDB.` outside any `persistence/` dir (`frontend/src/**/persistence/`). Mutating browser storage from random components is a war crime.

**`PrimeVueConfigImportOutsidePresetFile`** — Importing `primevue.config.*` or `PrimeVueConfig` types outside `src/generated/plugins/primevue.ts` is banned. Theming config lives in one file.

**`PiniaImport`** — Bans `import ... from 'pinia'` anywhere. Catablast does not use Pinia. State lives in codegen'd composables (per-resource, scoped) + URL params (view state). Cross-resource event coordination via the singleton bus exposed in `frontend/src/composables/bus.ts`.

Rationale: Pinia gives AI-written code too much rope. Codegen'd composables IS the store, scoped per-resource, hash-locked.

**`PrimeVueReinvented`** — Bans custom Vue components in `custom/components/` whose name matches a PrimeVue primitive: `Button`, `Card`, `Dialog`, `Drawer`, `Modal`, `Dropdown`, `Select`, `Checkbox`, `RadioButton`, `Slider`, `ProgressBar`, `Sidebar`, `Toolbar`, `Breadcrumb`, `Paginator`, `Skeleton`, `Toast`, `Tabs`, `TabView`, `Tab`, `DataTable`, `Tree`, `TreeTable`, `Calendar`, `DatePicker`, `Tooltip`. Use PrimeVue directly. Wrap with composition only when adding domain logic, and name the wrapper after the domain (`OrderActionsMenu`, not `Menu`).

### Routing + URL discipline

**`HardcodedRoutePath`** — Bans literal route paths in `router.push()`, `router.replace()`, `<router-link to="...">`, and `<a href="/...">` (where `/` starts a route, not external). Mandates named routes from `frontend/src/generated/router/route-names.ts`:

```ts
// BANNED
router.push('/users/42')
<router-link to="/orders" />

// REQUIRED
router.push({ name: 'users.detail', params: { id: 42 } })
<router-link :to="{ name: 'orders.list' }" />
```

Compiler-checked names. Rename a route → all callers fail to compile. Generated names are exhaustive: every route Blueprint declares ends up in the union type.

**`LocalModalState`** — Bans `ref(false)` patterns used for PrimeVue `<Dialog v-model:visible="...">`, `<Drawer>`, `<Sidebar>` modal-overlay state. Modals are URL state. Use codegen'd composables:

```ts
// BANNED
const showEdit = ref(false)
function openEdit() { showEdit.value = true }

// REQUIRED
const dialog = useQueryDialog('user-edit', { id })
dialog.open(42)   // sets ?dialog=user-edit&id=42; refresh-survival, back-button correct
```

Heuristic: detect `v-model:visible` with non-composable source. Whitelist for ephemeral tooltips/popovers if needed.

**`LocalListState`** — Bans local refs for pagination/sort/filter state on list views:

```ts
// BANNED
const page = ref(1)
const sort = ref('-created_at')

// REQUIRED — comes from URL via the codegen'd composable
const { data, page, sort, filter } = useUsersList()
```

Pagination/sort/filter ARE the URL contract (`?page&page_size&sort=-col&filter[col]=val`). Local-only state breaks refresh, share, back-button.

**`OptimisticUpdateInCustom`** — In custom code, bans patterns where a mutation handler updates local state before the server confirms. Custom code follows: action → pending state → await server → reconcile from response (or refetch). Codegen'd action helpers do this dance — custom MUST consume them, not roll its own.

Heuristic: flags mutation calls (`useUpdateX()`, `useCreateX()`, `useDeleteX()`) followed by local state mutation in the same handler block before `await`.

### Layout / page structure

**`PageShellRequired`** — Every Vue file under `frontend/src/pages/` (including `pages/generated/`) MUST have its template root be `<PageShell layout="...">`. No bare `<div>`/`<main>`/`<section>` roots. Rationale: every page goes through the layout enum, no orphan padding.

**`InlineLayoutProps`** — `<PageShell>` does not accept `padding`, `margin`, `gap`, `width`, `height` props. Layout is enum-locked: `cards`, `split`, `table`, `bleed`, `tabbed`. Devs pick a layout, layout owns the spacing.

Rule also bans inline `padding:` / `margin:` / `gap:` declarations in scoped `<style>` blocks at the top level of page components. They MUST come from layout primitives or token-driven utility classes.

**`LoadingSpinnerAfterFirstLoad`** — Warns on `v-if=".*[Ll]oading"` patterns inside template scoped to non-initial-load contexts. Once a composable's `data` ref is populated, refetch happens silently in the background (stale-while-revalidate). No skeleton flicker on poll/WS update.

The initial blocking-nav load uses the global progress bar (see `catalyst/doc/SPEC_FRONTEND_ROUTING.md`), not per-component spinners.

Heuristic-only; prefix `// @allow-spinner` to opt out for genuine async-action overlays (e.g. file upload progress).

### Template complexity

**`MaxLinesPerSfc`** — Configurable, default 600. Fails when a `.vue` SFC exceeds. Forces split into sub-components.

**`MaxLinesPerFn`** — Configurable, default 120. Warns when a TS function body exceeds.

**`MaxTemplateDepth`** — Configurable, default 5. Counts nested element depth in `<template>`. Banned: 6+ levels of `<div><div><div>...`. Forces extraction.

**`MaxTemplateLoc`** — Configurable, default 200. LOC inside the `<template>` block alone. Catches "200-line template, 30-line script" components that hide complexity in markup.

## Whitelist File

For genuine external-constant exceptions (schema.org URLs, SVG xmlns constants, CDN URLs), Blast emits `.rule_violations_whitelist`:

```
# file-glob : optional snippet substring
src/components/**/*.vue : schema.org
src/generated/icons.ts : xmlns
src/components/widgets/Tooltip.vue : v-model:visible
```

Configured via `app.ron` `fe_lint.whitelist_snippets`. Minimal by design — whitelisting is escape-hatch behavior, not extension mechanism.

The whitelist file `frontend/.rule_violations_whitelist` is codegen'd by `blast gen governor-plugin` and carries an `app.ron` content hash in its header. The user app's `build.rs` hard-fails if `app.ron` changed since last regen (same mechanism as all generated files — see `SPEC_STATE.md`).

## Violation Output

```
blast check
✗ 5 governor violations

frontend/src/pages/SettingsPage.vue:14
    [InlineStyle]  :style="{ padding: '16px' }"
    → use a class + scoped style with var(--app-space-lg)

frontend/src/components/UserCard.vue:32
    [HardcodedPx]  margin-top: 24px;
    → use var(--app-space-xl)

frontend/src/composables/useDashboard.ts:8
    [ConsoleLog]  console.log(value);
    → use import.meta.env.DEV wrapper

frontend/src/pages/UsersPage.vue:5
    [PageShellRequired]  template root is <main>; expected <PageShell layout="...">
    → wrap content in <PageShell layout="cards"> (or split/table/bleed/tabbed)

frontend/src/pages/OrderEditPage.vue:21
    [HardcodedRoutePath]  router.push('/orders')
    → router.push({ name: 'orders.list' })

Exit 1
```

Output grouped by violation type or file (configurable). Each violation shows file:line, rule name, snippet, suggested fix.

## Vite Plugin Wrapper

Blast emits `frontend/scripts/governor-plugin.js`:

```js
// GENERATED BY BLAST. Regenerated with `blast gen governor-plugin`.
import { execSync } from 'node:child_process';

export default function governorPlugin() {
    return {
        name: 'catablast-governor',
        enforce: 'pre',
        buildStart() {
            try {
                execSync('blast check', { stdio: 'inherit' });
            } catch (e) {
                throw new Error('blast check failed; see output above');
            }
        },
    };
}
```

Registered in `vite.config.ts`:

```ts
import governorPlugin from './scripts/governor-plugin.js';

export default defineConfig({
    plugins: [vue(), governorPlugin()],
});
```

Regenerated from `app.ron` `fe_lint` section via `blast gen governor-plugin` — never hand-edited.

## Implementation Notes (Blast-side)

- Files scanned: `frontend/src/**/*.{ts,vue,css}` — both `generated/` and `custom/` subtrees.
- Scanning is regex-based, line-by-line, for v1 rules. Template-depth and PageShell-required require minimal Vue SFC parsing (split `<template>` block, count tag-open/close stack); use a tight hand-rolled tokenizer, not a full AST parser.
- Per-file rule batching: read file once, run all applicable rules.
- Parallel file scan via Rayon.
- No file watching in v1 — Governor is a one-shot pre-build check. Watch mode defers to Vite HMR; lint runs only at build time.

## Relationship to Rust Clippy

Governor lints the FRONTEND. Backend Rust still goes through `cargo clippy`. Different tools, different scopes. No overlap.

## Anti-Patterns (for Blast maintainers)

- Hardcoding rule behavior into Blast. All thresholds and exempt lists come from `app.ron`'s `fe_lint` section.
- Writing a full Vue/TS AST parser for v1. Regex + minimal tag tokenization is adequate for the rules listed; upgrade if rules genuinely need AST.
- Emitting warnings for rules the user hasn't opted into. If a rule isn't in `app.ron`'s `fe_lint.deny_rules` list, it doesn't run.
- Silent failure modes. Governor always exits non-zero on violation; never "warn and continue."
- Skipping `generated/`. Lint everything. Generated code being clean is a forcing function on the codegen.

## Related Specs

- `SPEC_STATE.md` — `app.ron` fe_lint config source, hash-marker contract, Blueprint nav/pages section
- `catalyst/doc/SPEC_CSS.md` — CSS rules being enforced
- `catalyst/doc/SPEC_FRONTEND.md` — frontend layout scanned, layout enum, PageShell
- `catalyst/doc/SPEC_FRONTEND_ROUTING.md` — routing, modals as URL state, blocking nav
- `SPEC_CODEGEN.md` — Vite plugin wrapper emission, hash markers
- `SPEC_BLAST_COMMANDS.md` — `blast check` subcommand
