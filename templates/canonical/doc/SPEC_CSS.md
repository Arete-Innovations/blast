# SPEC_CSS

Monolithic SCSS design-token file + per-component scoped styles via `stylance`. Compiled by `cargo-leptos` (using `grass`). No node, no PostCSS, no Tailwind.

## Rules (Law)

1. **One monolithic `style/tokens.scss`** = single source of truth for all design values.
2. **Semantic token names.** `--app-color-brand`, `--app-color-danger`, `--app-fs-md`, `--app-space-lg`. NEVER `--blue`, `--red`, `--16px`.
3. **No inline colors** anywhere in Leptos components or scss. Use `var(--app-*)` or `var(--thaw-*)`.
4. **No `px` units** outside niches:
   - `0.0625rem` allowed for hairline borders (1px-equivalent in rem).
   - `@media` query breakpoints (viewport-pinned, not scaled).
   - Three exempt files: `style/tokens.scss`, `style/base.scss`, the thaw theme override file.
5. **`rem` + responsive** (`clamp()`, `vw`) for everything else. Root scales with viewport.
6. **Per-component styles via stylance** (`.module.scss` files with hashed classnames). No global styles outside `tokens.scss` / `base.scss`.
7. **OKLCH only** — modern browsers, no fallbacks.

## File layout

```
style/
├── main.scss      cargo-leptos entry; @use's tokens + base
├── tokens.scss    design tokens (--app-*) — single source of truth
└── base.scss      reset + root font scaling + body defaults
```

Per-component scoped styles live alongside their `.rs` files:

```
src/transport/leptos/components/
├── page_shell.rs
├── page_shell.module.scss   <-- stylance hashes the classnames
└── error_banner.module.scss
```

Stylance scans for `.module.scss` files at build time and emits a generated module per source `.rs` exposing typed constants for each class (e.g. `style::page_shell::CARD`).

## Theme structure

`tokens.scss` is split into three sections:

1. **Palette knobs** (~9 vars) at the top of `:root`:
   - `--app-brand-hue`, `--app-brand-chroma`
   - `--app-neutral-hue`, `--app-neutral-chroma`
   - `--app-status-chroma`, `--app-info-hue`, `--app-success-hue`, `--app-warning-hue`, `--app-danger-hue`

2. **Light block** — `:root { ... }` declares every `--app-*` token. Color ramps via OKLCH.

3. **Dark block** — `@media (prefers-color-scheme: dark) { :root { ... } }` re-declares **every** token. Colors flip; spacing/radius/font/motion/z-index keep identical values.

Default behavior follows OS preference. A dark-mode toggle is supported via thaw's `ConfigProvider` (decision #26: OS preference + manual toggle).

## Token categories

```scss
:root {
    // ── typography (rem-scaled) ──────────────────────
    --app-fs-2xs: 0.75rem;
    --app-fs-xs:  0.875rem;
    --app-fs-md:  1rem;
    --app-fs-lg:  1.125rem;
    --app-fs-xl:  1.25rem;

    // ── responsive headings ──────────────────────────
    --app-fs-h1-resp: clamp(1.5rem, 3vw, 2.25rem);
    --app-fs-h2-resp: clamp(1.25rem, 2.5vw, 1.875rem);

    // ── spacing ──────────────────────────────────────
    --app-space-xs: 0.25rem;
    --app-space-sm: 0.5rem;
    --app-space-md: 0.75rem;
    --app-space-lg: 1rem;
    --app-space-xl: 1.5rem;

    // ── radius ───────────────────────────────────────
    --app-radius-sm: 0.25rem;
    --app-radius-md: 0.5rem;

    // ── color (OKLCH) ────────────────────────────────
    --app-color-bg: oklch(0.98 0 0);
    --app-color-fg: oklch(0.18 0 0);
    --app-color-brand: oklch(0.55 var(--app-brand-chroma) var(--app-brand-hue));
    --app-color-danger: oklch(0.55 0.18 25);
    --app-color-success: oklch(0.55 0.16 145);

    // ── z-index ──────────────────────────────────────
    --app-z-modal: 80;
    --app-z-toast: 120;
}
```

## Root font scaling

```scss
html {
    font-size: clamp(14px, calc(100vw / 120), 32px);
}
```

Root font scales 14px → 32px based on viewport width. Everything downstream in `rem` auto-scales. No media queries needed for responsive typography.

## Per-component pattern (stylance)

```rust
use stylance::import_style;

import_style!(style, "page_shell.module.scss");

#[component]
pub fn PageShell(layout: PageLayout, children: Children) -> impl IntoView {
    view! {
        <main class=style::SHELL data-layout=layout.as_str()>
            {children()}
        </main>
    }
}
```

```scss
// page_shell.module.scss
.shell {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-md);

    &[data-layout="cards"] {
        padding: var(--app-space-xl);
    }

    &[data-layout="bleed"] {
        padding: 0;
    }
}
```

Stylance hashes `.shell` to `.shell_<hash>` so component-local class names never collide globally.

## Thaw integration

Thaw provides `--thaw-*` CSS custom properties (e.g. `--thaw-color-brand-foreground-1`). Catablast's tokens map onto thaw's via the thaw `Theme` provider in `<App>`:

```rust
let theme = create_signal(Theme::dark());
view! {
    <ConfigProvider theme>
        <Routes ...>
    </ConfigProvider>
}
```

The thaw preset can be customized to remap `--thaw-*` → `var(--app-*)` so swapping the preset swaps the whole palette. Thaw components consume the remapped tokens transparently.

## Anti-patterns

**Inline `style=` attribute:**
```rust
<div style=format!("padding: 16px;")> // BANNED
```
Use a class + module scss with `var(--app-space-lg)`.

**Hex outside theme override:**
```scss
.button { background: #7c3aed; } // BANNED
```
Use a token.

**Px outside niches:**
```scss
.box { margin: 16px; } // BANNED
```
Use `var(--app-space-lg)`.

**Raw rem outside `tokens.scss`:**
```scss
.box { padding: 1rem; } // BANNED
```
Use `var(--app-space-lg)`.

**Tailwind / utility class sprinkling:**
```rust
<div class="p-4 m-2 text-sm"> // BANNED
```
Define a semantic class in `.module.scss` and style with tokens.

## Lint enforcement (planned)

A `LEPTOS:*` rule family in `build.rs` will enforce:
- `LEPTOS:1` — no inline `style=` attributes in `view!` macros
- `LEPTOS:2` — no hex / rgb / hsl outside `style/` dir
- `LEPTOS:3` — no `px` outside niches
- `LEPTOS:4` — every page component wraps top-level view in `<PageShell layout=...>`

Currently scaffolded in phase 4 but not active.

## Related specs

- `SPEC_LEPTOS.md` — Leptos UI integration
- `blast/doc/SPEC_GOVERNOR.md` — DELETED. Replaced by `LEPTOS:*` family in canonical's `build.rs`.
