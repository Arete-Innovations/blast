# SPEC_GOVERNOR

Frontend lint engine. Lives in `blast::governor`. Invoked as `blast check`. Written in Rust; wrapped as a Vite plugin for `prebuild`/`predev` hooks.

## Why Governor

Engine governors limit RPM to prevent over-revving. Catablast's Governor limits FE code to the rules that keep the design system, type safety, and responsive scaling intact. Violations fail the build.

Catablast does NOT use a JS-based linter (`.mjs` script). Rule engine is in Rust, inside Blast itself. Reasons:
- Single binary, single language
- Rust regex outpaces Node regex loops on large file sets
- Rules change via blueprint, not by hand-editing emitted JS
- Debugging rule logic = read Rust, not obscure JS

## Invocation

```
blast check                         # lints frontend/ source tree
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
    hairline_border_rem: "0.0625rem",
    exempt_color_files: ["src/plugins/primevue.ts"],
    exempt_px_files: [
        "src/plugins/primevue.ts",
        "src/styles/tokens.css",
        "src/styles/base.css",
    ],
    whitelist_snippets: ["schema.org"],
    deny_rules: [
        ConsoleLog,
        InlineStyle,
        RawRemOutsideTokens,
        TypeAny,
        TsIgnore,
        SilentFallback,
        IconClassOutsideIconsFile,
        RawColorOutsidePreset,
        HardcodedPx,
    ],
),
```

Blast reads `storage/blast/state/app.ron` directly and applies the configured rules. There is no `target/blueprint/fe_lint.json` IR — the `catalyst_blueprint` DSL crate is deleted.

## Rules

### Color discipline

**`RawColorOutsidePreset`** — Regex bans `#[0-9a-fA-F]{3,8}`, `rgb(`, `rgba(`, `hsl(`, `hsla(` outside files listed in `.exempt_color_files`.

Rationale: colors must flow through the design token system (PrimeVue preset or `tokens.css`).

Default exempt: `src/plugins/primevue.ts`.

### Px discipline

**`HardcodedPx`** — Regex bans `\d+(\.\d+)?px` outside files listed in `.exempt_px_files`. Line-level exceptions: `@media` queries, `rootMargin:` (IntersectionObserver API), explicit hairline-border escape (`0.0625rem` in regex allow-list).

Rationale: `px` doesn't scale with root font size; breaks responsive scaling on 4K / high-DPI.

Default exempt files: `src/plugins/primevue.ts`, `src/styles/tokens.css`, `src/styles/base.css`.

**`RawRemOutsideTokens`** — Regex bans literal `\d+(\.\d+)?rem` in files other than `tokens.css`. Forces `var(--app-*)` token use in components.

Line-level exceptions: `grid-template-columns: minmax(...)`, `@media` queries, `letter-spacing:`, `filter: blur(...)`, `backdrop-filter:`, `background-size:`, `box-shadow:` — cases where bare rem has meaning that tokens can't express cleanly.

### Inline styles

**`InlineStyle`** — Bans `style="..."` attributes and `:style="..."` directives on HTML/Vue templates.

Rationale: style belongs in `<style scoped>` blocks, not strewn through markup.

### TypeScript discipline

**`TypeAny`** — Bans `: any` type annotations. Escape hatch: `@allow-any` comment on the line (discouraged).

**`TsIgnore`** — Bans `@ts-ignore` and `@ts-nocheck` comments. Force proper typing or explicit `any` with allow.

**`SilentFallback`** — Bans patterns like `something || '...'` on potentially-falsy values (heuristic), `?? {}`, `?? []` when the empty default silently swallows nil. Rationale: null-hiding defaults mask bugs.

Heuristic is imperfect; line-level `@allow-fallback` comment escape.

### Code hygiene

**`ConsoleLog`** — Bans `console.log`, `console.warn`, `console.error` in committed source. Use `import.meta.env.DEV && console.log(...)` or equivalent dev-only wrappers.

**`IconClassOutsideIconsFile`** — Icon class names (`pi pi-user`, `ph ph-check`, `fa fa-bell`, etc.) may only appear in `src/icons.ts`. Rationale: centralized icon registry prevents drift and enables swap.

Class name patterns configured via `.icon_class_patterns([...])` on FeLintConfig.

### LOC limits

**`MaxLinesPerSfc`** — Configurable, default 600. Warns (or fails) when a `.vue` SFC exceeds.

**`MaxLinesPerFn`** — Configurable, default 120. Warns when a TS function body exceeds.

### PrimeVue config isolation

**`PrimeVueConfigImportOutsidePresetFile`** — Importing `primevue.config.*` or `PrimeVueConfig` types outside `src/plugins/primevue.ts` is banned. Rationale: theming config must live in one file.

## Whitelist File

For genuine external-constant exceptions (schema.org URLs, SVG xmlns constants, CDN URLs), Blast emits `.rule_violations_whitelist`:

```
# file-glob : optional snippet substring
src/components/**/*.vue : schema.org
src/icons.ts : xmlns
```

Configured via `app.ron` `fe_lint.whitelist_snippets`. Minimal by design — whitelisting is escape-hatch behavior, not extension mechanism.

The whitelist file `frontend/.rule_violations_whitelist` is codegen'd by `blast gen governor-plugin` and carries an `app.ron` content hash in its header. The user app's `build.rs` hard-fails if `app.ron` changed since last regen (same mechanism as all generated files — see `SPEC_STATE.md`).

## Violation Output

```
blast check
✗ 3 governor violations

frontend/src/components/NavBar.vue:14
    [InlineStyle]  :style="{ padding: '16px' }"
    → use a class + scoped style

frontend/src/views/AccountView.vue:32
    [HardcodedPx]  margin-top: 24px;
    → use var(--app-space-xl) or similar

frontend/src/utils/format.ts:8
    [ConsoleLog]  console.log(value);
    → use import.meta.env.DEV wrapper

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

- Files scanned: `frontend/src/**/*.{ts,vue,css}` (configurable via blueprint)
- Scanning is regex-based, line-by-line. No AST parsing for v1. Rust regex crate is fast enough for 10K-LOC repos.
- Per-file rule batching: read file once, run all applicable rules.
- Parallel file scan via Rayon.
- No file watching in v1 — Governor is a one-shot pre-build check. Watch mode defers to Vite HMR; lint runs only at build time.

## Relationship to Rust Clippy

Governor lints the FRONTEND. Backend Rust still goes through `cargo clippy`. Different tools, different scopes. No overlap.

## Anti-Patterns (for Blast maintainers)

- Hardcoding rule behavior into Blast. All thresholds and exempt lists come from `app.ron`'s `fe_lint` section.
- Writing an AST parser for Vue/TS v1. Regex is adequate; upgrade if rules genuinely need AST.
- Emitting warnings for rules the user hasn't opted into. If a rule isn't in blueprint's `deny_rule()` list, it doesn't run.
- Silent failure modes. Governor always exits non-zero on violation; never "warn and continue."

## Related Specs

- `SPEC_STATE.md` — `app.ron` fe_lint config source, hash-marker contract
- `catalyst/doc/SPEC_CSS.md` — CSS rules being enforced
- `catalyst/doc/SPEC_FRONTEND.md` — frontend layout scanned
- `SPEC_CODEGEN.md` — Vite plugin wrapper emission, hash markers
- `SPEC_BLAST_COMMANDS.md` — `blast check` subcommand
