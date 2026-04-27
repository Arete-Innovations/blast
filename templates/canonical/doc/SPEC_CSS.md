# SPEC_CSS

Monolithic design-token file + scoped SFC styles. No Tailwind. No utility-class layer. Build-time lint enforces the rules.

## Rules (Law)

1. **One monolithic `tokens.css`** = single source of truth for all design values.
2. **Semantic token names.** `--app-color`, `--app-accent`, `--app-warning`, `--app-fs-md`, `--app-space-lg`. NEVER `--blue`, `--red`, `--green`, `--16px`, etc.
3. **No inline colors** anywhere in SFCs (`.vue`), TypeScript, or CSS. Use `var(--app-*)` or `var(--p-*)`.
4. **No `px` units** outside niches:
   - `0.0625rem` allowed for hairline borders (1px-equivalent in rem)
   - `@media` query breakpoints (viewport-pinned, not scaled)
   - Three specific files exempt: `src/plugins/primevue.ts`, `src/styles/tokens.css`, `src/styles/base.css`
5. **`rem` + responsive** (`clamp()`, `vw`) for everything else. Root scales with viewport.
6. **Scoped styles with `@layer app`.** No global styles outside `base.css` and `tokens.css`.
7. **Scoped `<style>` blocks get the same hammer as global CSS.** All color/px/rem rules apply identically inside per-SFC `<style scoped>`. There is no "I'm in a scoped block so I can hardcode" exception. Inline `style=` and `:style=` directives also subject to the same rules — actually banned outright (`InlineStyle` rule), but if they sneak through they still fail color/px checks.
8. **Enforcement via `blast check` / Governor** (see `blast/doc/SPEC_GOVERNOR.md`). Fail the build on violation. Lints both `generated/` and `custom/` subtrees.

## Root Font Scaling

```css

html {
    font-size: clamp(14px, calc(100vw / 120), 32px);
}
```

Root font scales 14px → 32px based on viewport width. Everything downstream in `rem` auto-scales. No media queries needed for responsive typography or spacing.

On a 4K monitor at 100% browser zoom, root hits 32px; UI reads comfortably without 200% zoom. On mobile, root clamps to 14px. No breakpoint gymnastics.

## Token Categories

```css

@layer app {
    :root {
        
        --app-fs-2xs: 0.75rem;
        --app-fs-xs:  0.875rem;
        --app-fs-sm:  0.9375rem;
        --app-fs-md:  1rem;
        --app-fs-lg:  1.125rem;
        --app-fs-xl:  1.25rem;
        --app-fs-2xl: 1.5rem;
        --app-fs-3xl: 1.875rem;
        --app-fs-4xl: 2.25rem;
        --app-fs-5xl: 3rem;

        
        --app-fs-body-resp:    clamp(0.9375rem, 1.5vw, 1.125rem);
        --app-fs-h1-resp:      clamp(1.5rem, 3vw, 2.25rem);
        --app-fs-h2-resp:      clamp(1.25rem, 2.5vw, 1.875rem);
        --app-fs-display-lg:   clamp(2rem, 5vw, 3.5rem);

        
        --app-space-0:   0;
        --app-space-xs:  0.25rem;
        --app-space-sm:  0.5rem;
        --app-space-md:  0.75rem;
        --app-space-lg:  1rem;
        --app-space-xl:  1.5rem;
        --app-space-2xl: 2rem;
        --app-space-3xl: 3rem;
        --app-space-4xl: 4rem;
        --app-space-5xl: 5rem;

        
        --app-icon-xs: 0.875rem;
        --app-icon-sm: 1rem;
        --app-icon-md: 1.25rem;
        --app-icon-lg: 1.5rem;
        --app-icon-xl: 2rem;

        
        --app-container-xs: 26rem;
        --app-container-sm: 40rem;
        --app-container-md: 56rem;
        --app-container-lg: 72rem;
        --app-container-xl: 90rem;

        
        --app-pad-section-sm: clamp(2rem, 5vw, 4rem);
        --app-pad-section-md: clamp(4rem, 10vw, 7.5rem);

        
        --app-z-content:   1;
        --app-z-nav:       10;
        --app-z-dropdown:  50;
        --app-z-modal:     80;
        --app-z-overlay:   100;
        --app-z-toast:     120;

        
        --app-transition-fast:   120ms ease;
        --app-transition-normal: 200ms ease;
        --app-transition-slow:   320ms ease;

        
        --app-radius-sm: 0.25rem;
        --app-radius-md: 0.5rem;
        --app-radius-lg: 0.75rem;
        --app-radius-xl: 1rem;
        --app-radius-full: 9999px;

        
        
    }
}
```

Edit this file directly — it's the source of truth. Never inline a value in an SFC.

## Color Tokens via PrimeVue

Colors live in the PrimeVue preset (`src/plugins/primevue.ts`). That file is the only location allowed to contain hex values; Governor exempts it explicitly. App tokens map onto PrimeVue tokens:

```ts

export const PRESET_SEMANTIC = definePreset(Aura, {
    semantic: {
        primary: {
            50:  '#faf5ff',
            500: '#7c3aed',
            900: '#4c1d95',
        },
        colorScheme: {
            light: {
                surface: {
                    0:   '#ffffff',
                    100: '#f5f5f5',
                },
                text: { color: '{surface.950}' },
                content: { background: '{surface.0}', borderColor: '{surface.200}' },
            },
            dark: {  },
        },
    },
});
```

SFCs reference colors as `var(--p-text-color)`, `var(--p-content-border-color)`, `var(--p-primary-color)` — PrimeVue-emitted CSS variables. Since PrimeVue tokens flow from the preset, changing the preset swaps the whole palette.

For colors NOT provided by PrimeVue (app-specific e.g. per-platform chart colors), define `--app-color-*` tokens directly in `tokens.css`.

## CSS Layer Order

```ts

import PrimeVue from 'primevue/config';
app.use(PrimeVue, {
    theme: {
        preset: PRESET_SEMANTIC,
        options: {
            cssLayer: { name: 'primevue', order: 'reset, primevue, app' },
        },
    },
});
```

App layer comes last → app styles override PrimeVue's. `reset` is a minimal CSS reset first; `primevue` is PrimeVue internals; `app` is Catablast SFC + token styles.

## SFC Style Pattern

```vue
<script setup lang="ts">
defineProps<{ order: OrderPublic }>();
</script>

<template>
    <article class="card">
        <header>
            <h3 class="title">{{ order.display_name }}</h3>
            <span class="status" :data-status="order.status">{{ order.status }}</span>
        </header>
        <footer>
            <time>{{ formatRelative(order.updated_at) }}</time>
        </footer>
    </article>
</template>

<style scoped>
@layer app {
    .card {
        display: flex;
        flex-direction: column;
        gap: var(--app-space-md);
        padding: var(--app-space-xl);
        border: 0.0625rem solid var(--p-content-border-color);
        border-radius: var(--app-radius-md);
        background: var(--p-content-background);
    }

    .title {
        font-size: var(--app-fs-h2-resp);
        color: var(--p-text-color);
    }

    .status {
        font-size: var(--app-fs-sm);
        color: var(--p-text-muted-color);
    }

    .status[data-status="failed"] {
        color: var(--app-color-danger);
    }
</style>
```

All values from tokens. Semantic class names (`.card`, `.status`), not color-named (`.red-card`).

## Responsive

Use `clamp()` + `vw` tokens instead of media queries where possible:

```css

font-size: var(--app-fs-h1-resp);    
padding: var(--app-pad-section-md);  
```

Media queries allowed but rare:

```css

@media (min-width: 768px) {
    .grid { grid-template-columns: 1fr 1fr; }
}
```

## Exempt Files

Three files are allowed to violate the rules (and lint whitelists them):

- `src/plugins/primevue.ts` — PrimeVue preset, only file with hex colors
- `src/styles/tokens.css` — defines raw rem values
- `src/styles/base.css` — defines root font-size clamp

A `.rule_violations_whitelist` file allows additional per-pattern exceptions (e.g. schema.org URLs, SVG xmlns constants) — configured in `storage/blast/state/app.ron` under `fe_lint`.

## Anti-Patterns

**Inline style attribute:**
```vue

<div :style="{ padding: '16px', color: '#333' }">
```

Banned entirely. Use classes.

**Hex in SFC:**
```css

.button { background: #7c3aed; }
```

Use token.

**Px outside niches:**
```css

.box { margin: 16px; }
```

Use `var(--app-space-lg)`.

**Raw rem outside tokens.css:**
```css

.box { padding: 1rem; }
```

Even rem is disallowed outside `tokens.css`. Use `var(--app-space-lg)`. Governor catches this.

**Color-named classes:**
```css

.red-text { color: red; }
```

Use semantic names: `.error-text { color: var(--app-color-danger); }`.

**Utility class sprinkling:**
```vue

<div class="p-4 m-2 text-sm text-gray-700">
```

Banned pattern. Define a semantic class and style it with tokens.

## Seeded From

The pattern is modeled after `/home/tragdate/codumeu/upnumbers/frontend/` — specifically `src/styles/tokens.css`, `src/styles/base.css`, `src/plugins/primevue.ts`. Blast scaffolds new apps with these seed files adapted to Catablast's naming.

## Related Specs

- `SPEC_FRONTEND.md` — Vue/TS/Vite integration
- `blast/doc/SPEC_GOVERNOR.md` — enforcement rules
- `SPEC_CONFIG.md` — `app.ron` fe_lint section for rule configuration
