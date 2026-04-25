
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{BlastError, BlastResult};

const TOKENS_RELATIVE_PATH: &str = "frontend/src/styles/tokens.css";
const BASE_RELATIVE_PATH: &str = "frontend/src/styles/base.css";
const PRIMEVUE_RELATIVE_PATH: &str = "frontend/src/plugins/primevue.ts";

const TOKENS_CSS: &str = r#"@layer app {
  :root {
    --app-font-mono: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
    --app-font-sans: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;

    --app-fs-2xs: 0.75rem;
    --app-fs-xs:  0.8125rem;
    --app-fs-sm:  0.875rem;
    --app-fs-md:  1rem;
    --app-fs-lg:  1.125rem;
    --app-fs-xl:  1.25rem;
    --app-fs-2xl: 1.5rem;
    --app-fs-3xl: 1.75rem;
    --app-fs-4xl: 2.25rem;
    --app-fs-5xl: 3.5rem;

    --app-space-0:   0;
    --app-space-3xs: 0.0625rem;
    --app-space-2xs: 0.125rem;
    --app-space-xs:  0.25rem;
    --app-space-sm:  0.375rem;
    --app-space-md:  0.5rem;
    --app-space-lg:  0.75rem;
    --app-space-xl:  1rem;
    --app-space-2xl: 1.25rem;
    --app-space-3xl: 1.5rem;
    --app-space-4xl: 2rem;
    --app-space-5xl: 2.5rem;
    --app-space-6xl: 3rem;
    --app-space-7xl: 4rem;

    --app-icon-xs:  1rem;
    --app-icon-sm:  1.25rem;
    --app-icon-md:  1.5rem;
    --app-icon-lg:  1.75rem;
    --app-icon-xl:  2rem;
    --app-icon-2xl: 2.5rem;

    --app-container-xs: 28rem;
    --app-container-sm: 32rem;
    --app-container-md: 40rem;
    --app-container-lg: 50rem;
    --app-container-xl: 60rem;
    --app-container-2xl: 72rem;

    --app-fs-body-resp:  clamp(0.9375rem, 1.5vw, 1.125rem);
    --app-fs-sub-resp:   clamp(1.125rem, 1.7vw, 1.375rem);
    --app-fs-h3-resp:    clamp(1.25rem, 2.5vw, 1.75rem);
    --app-fs-h2-resp:    clamp(1.5rem, 2.6vw, 2rem);
    --app-fs-h1-resp:    clamp(1.5rem, 3vw, 2.25rem);
    --app-fs-display-sm: clamp(1.75rem, 4vw, 2.75rem);
    --app-fs-display-lg: clamp(2.25rem, 6vw, 4.5rem);

    --app-pad-section-sm: clamp(3rem, 8vw, 5rem);
    --app-pad-section-md: clamp(4rem, 10vw, 7.5rem);
    --app-pad-section-lg: clamp(5rem, 12vw, 10rem);

    --app-z-content: 1;
    --app-z-sidebar: 20;
    --app-z-topbar:  30;
    --app-z-overlay: 100;
    --app-z-toast:   120;

    --app-transition-fast: 0.12s ease;
    --app-transition-med:  0.18s ease;
    --app-transition-slow: 0.32s ease;

    --app-radius-sm:   0.25rem;
    --app-radius-md:   0.5rem;
    --app-radius-lg:   0.75rem;
    --app-radius-xl:   1rem;
    --app-radius-pill: 999px;
  }
}
"#;

const BASE_CSS: &str = r#"@layer reset {
  *,
  *::before,
  *::after {
    box-sizing: border-box;
  }

  html {
    scroll-behavior: smooth;
    font-size: clamp(14px, calc(100vw / 120), 32px);
  }

  html,
  body,
  #app {
    margin: 0;
    padding: 0;
    min-height: 100vh;
    font-family: var(--app-font-sans);
    background: var(--p-content-background);
    color: var(--p-text-color);
  }

  @media (prefers-reduced-motion: reduce) {
    html { scroll-behavior: auto; }
  }

  a {
    color: inherit;
    text-decoration: none;
  }

  button {
    font-family: inherit;
  }

  body.app-scroll-locked {
    overflow: hidden;
    touch-action: none;
  }
}
"#;

const PRIMEVUE_TS: &str = r#"import type { App } from 'vue'
import PrimeVueConfig from 'primevue/config'
import Aura from '@primevue/themes/aura'
import { definePreset } from '@primevue/themes'

const PRESET_SEMANTIC = definePreset(Aura, {
  semantic: {
    primary: {
      50:  '{violet.50}',
      100: '{violet.100}',
      200: '{violet.200}',
      300: '{violet.300}',
      400: '{violet.400}',
      500: '{violet.500}',
      600: '{violet.600}',
      700: '{violet.700}',
      800: '{violet.800}',
      900: '{violet.900}',
      950: '{violet.950}'
    },
    colorScheme: {
      light: {
        surface: {
          0:   '#ffffff',
          50:  '{slate.50}',
          100: '{slate.100}',
          200: '{slate.200}',
          300: '{slate.300}',
          400: '{slate.400}',
          500: '{slate.500}',
          600: '{slate.600}',
          700: '{slate.700}',
          800: '{slate.800}',
          900: '{slate.900}',
          950: '{slate.950}'
        }
      },
      dark: {
        surface: {
          0:   '#0a0a0a',
          50:  '{slate.950}',
          100: '{slate.900}',
          200: '{slate.800}',
          300: '{slate.700}',
          400: '{slate.600}',
          500: '{slate.500}',
          600: '{slate.400}',
          700: '{slate.300}',
          800: '{slate.200}',
          900: '{slate.100}',
          950: '{slate.50}'
        }
      }
    }
  }
})

export default function installPrimeVue(app: App): void {
  app.use(PrimeVueConfig, {
    theme: {
      preset: PRESET_SEMANTIC,
      options: {
        cssLayer: { name: 'primevue', order: 'reset, primevue, app' }
      }
    }
  })
}
"#;

pub struct ScaffoldOutcome {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

pub fn run(project_root: &Path) -> BlastResult<ScaffoldOutcome> {
    let mut written = Vec::new();
    let mut skipped = Vec::new();
    let targets: [(&str, &str); 3] = [
        (TOKENS_RELATIVE_PATH, TOKENS_CSS),
        (BASE_RELATIVE_PATH, BASE_CSS),
        (PRIMEVUE_RELATIVE_PATH, PRIMEVUE_TS),
    ];
    for (rel, body) in targets.iter() {
        let target = project_root.join(rel);
        match write_if_absent(&target, body)? {
            true => written.push(target),
            false => skipped.push(target),
        }
    }
    Ok(ScaffoldOutcome { written, skipped })
}

fn write_if_absent(target: &Path, body: &str) -> BlastResult<bool> {
    if target.exists() {
        return Ok(false);
    }
    let parent = target
        .parent()
        .ok_or_else(|| BlastError::Invalid(format!("scaffold path has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(target, body)?;
    Ok(true)
}
