//! Frontend runtime scaffold — ships the static Vue/TS pieces that every
//! Catablast app boots with: page shell, global progress bar + composable,
//! the blocking-nav router guard installer, the router index template, and
//! the index.html pre-mount shell.
//!
//! These files are SCAFFOLD seeds, not per-resource codegen. They are
//! `write_if_absent` on `blast new` and on `blast gen all` — the user is
//! free to tweak them after first emission and Blast will not stomp the
//! changes. (Matches the pattern in `frontend_scaffold.rs`.)
//!
//! The `frontend/index.html` template uses a templating placeholder
//! (`{{APP_NAME}}`) substituted at emission time so the document title
//! reflects the consumer app's name. Every other file is byte-stable.
//!
//! This module is intentionally header-marker-free: scaffold seeds aren't
//! state-driven, so there's no source state hash to embed. (The codegen
//! pipeline that emits per-resource pages and router config — lane 4 in
//! Wave 7b — owns the hash-marked outputs that live alongside these
//! seeds.)

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::fe_runtime_composables::{CHANNEL_TS, DIALOG_TS, DRAWER_TS, URL_TS};
use crate::error::{BlastError, BlastResult};

const APP_NAME_TOKEN: &str = "{{APP_NAME}}";

const PAGE_SHELL_RELATIVE: &str = "frontend/src/components/PageShell.vue";
const GLOBAL_PROGRESS_BAR_RELATIVE: &str = "frontend/src/components/GlobalProgressBar.vue";
const GLOBAL_PROGRESS_TS_RELATIVE: &str = "frontend/src/composables/global-progress.ts";
const INSTALL_BLOCKING_NAV_RELATIVE: &str = "frontend/src/router/install-blocking-nav.ts";
const ROUTER_INDEX_RELATIVE: &str = "frontend/src/router/index.ts";
const INDEX_HTML_RELATIVE: &str = "frontend/index.html";
const MAIN_TS_RELATIVE: &str = "frontend/src/main.ts";
const URL_TS_RELATIVE: &str = "frontend/src/composables/url.ts";
const DIALOG_TS_RELATIVE: &str = "frontend/src/composables/dialog.ts";
const DRAWER_TS_RELATIVE: &str = "frontend/src/composables/drawer.ts";
const CHANNEL_TS_RELATIVE: &str = "frontend/src/composables/channel.ts";

pub const PAGE_SHELL_VUE: &str = PAGE_SHELL_VUE_BODY;
pub const GLOBAL_PROGRESS_BAR_VUE: &str = GLOBAL_PROGRESS_BAR_VUE_BODY;
pub const GLOBAL_PROGRESS_TS: &str = GLOBAL_PROGRESS_TS_BODY;
pub const INSTALL_BLOCKING_NAV_TS: &str = INSTALL_BLOCKING_NAV_TS_BODY;
pub const ROUTER_INDEX_TS: &str = ROUTER_INDEX_TS_BODY;
pub const INDEX_HTML_TEMPLATE: &str = INDEX_HTML_BODY;
pub const MAIN_TS: &str = MAIN_TS_BODY;

const PAGE_SHELL_VUE_BODY: &str = r#"<script setup lang="ts">
// PageShell — every page wraps content in this. Layout is enum-locked;
// the layout chosen owns the spacing. No padding/margin/gap props are
// accepted — pick a layout, layout owns the spacing (per SPEC_FRONTEND).
//
// Layouts:
//   cards   — default; padded; gap between sections
//   split   — master-detail; rail attaches to sidebar; right-padded only
//   table   — zero padding; full viewport height; child table scrolls
//   bleed   — zero everything; full viewport; component owns chrome
//   tabbed  — tab container; child <RouterView> renders inside

export type PageLayout = 'cards' | 'split' | 'table' | 'bleed' | 'tabbed'

defineProps<{ layout: PageLayout }>()
</script>

<template>
  <section class="page-shell" :data-layout="layout">
    <header v-if="$slots.header" class="page-shell-header">
      <slot name="header" />
    </header>
    <div class="page-shell-body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
@layer app {
  .page-shell {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    background: var(--p-content-background);
    color: var(--p-text-color);
  }

  .page-shell-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--app-space-md);
    padding-block: var(--app-space-md);
    padding-inline: var(--app-space-md);
    border-bottom: 0.0625rem solid var(--p-content-border-color);
  }

  .page-shell-body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
  }

  /* cards — default. padding all around, gap between stacked sections */
  .page-shell[data-layout='cards'] .page-shell-body {
    gap: var(--app-space-md);
    padding: var(--app-space-md);
  }

  /* split — master-detail. flex row that stacks on narrow viewport */
  .page-shell[data-layout='split'] .page-shell-body {
    display: flex;
    flex-direction: row;
    gap: var(--app-space-md);
    padding-inline-end: var(--app-space-md);
    padding-inline-start: 0;
  }

  @media (max-width: 48rem) {
    .page-shell[data-layout='split'] .page-shell-body {
      flex-direction: column;
      padding-inline-end: var(--app-space-md);
      padding-inline-start: var(--app-space-md);
    }
  }

  /* table — full viewport, zero padding, child owns scroll */
  .page-shell[data-layout='table'] {
    height: 100vh;
    overflow: hidden;
  }

  .page-shell[data-layout='table'] .page-shell-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }

  /* bleed — zero everything, full viewport */
  .page-shell[data-layout='bleed'] {
    height: 100vh;
    overflow: hidden;
  }

  .page-shell[data-layout='bleed'] .page-shell-header {
    display: none;
  }

  .page-shell[data-layout='bleed'] .page-shell-body {
    flex: 1 1 auto;
  }

  /* tabbed — tab container; child <RouterView> picks its own layout */
  .page-shell[data-layout='tabbed'] .page-shell-body {
    padding-block-start: 0;
    padding-block-end: var(--app-space-md);
    padding-inline: var(--app-space-md);
    gap: var(--app-space-md);
  }
}
</style>
"#;

const GLOBAL_PROGRESS_BAR_VUE_BODY: &str = r#"<script setup lang="ts">
// GlobalProgressBar — top-of-viewport indeterminate bar for blocking nav
// transitions. Listens to the global-progress composable. Visible from
// the FIRST nav (cold load) so users see "the app is loading" with no
// blank screen. See SPEC_FRONTEND_ROUTING.md for the blocking-nav model.

import { useGlobalProgress } from '@/composables/global-progress'

const { isActive } = useGlobalProgress()
</script>

<template>
  <div
    class="global-progress-bar"
    role="progressbar"
    aria-live="polite"
    aria-label="Loading"
    :data-active="isActive"
  >
    <div class="global-progress-bar-fill" />
  </div>
</template>

<style scoped>
@layer app {
  .global-progress-bar {
    position: fixed;
    top: 0;
    inset-inline: 0;
    height: 0.1875rem;
    overflow: hidden;
    z-index: var(--app-z-overlay);
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--app-transition-fast);
  }

  .global-progress-bar[data-active='true'] {
    opacity: 1;
  }

  .global-progress-bar-fill {
    width: 40%;
    height: 100%;
    background: var(--p-primary-color);
    transform: translateX(-100%);
    will-change: transform;
  }

  .global-progress-bar[data-active='true'] .global-progress-bar-fill {
    animation: global-progress-bar-slide 1.1s linear infinite;
  }

  @keyframes global-progress-bar-slide {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(350%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .global-progress-bar-fill {
      animation: none;
    }
  }
}
</style>
"#;

const GLOBAL_PROGRESS_TS_BODY: &str = r#"// global-progress — module-scoped active-task counter backing the
// GlobalProgressBar. Multiple concurrent navs/fetches each call start();
// finish() decrements; isActive is true when count > 0. cancel() force-
// clears (used on hard router error). No external state library — this
// IS the state. See SPEC_FRONTEND_ROUTING.md.

import { computed, ref } from 'vue'
import type { ComputedRef } from 'vue'

const active_count = ref(0)
const is_active = computed<boolean>(() => active_count.value > 0)

export interface GlobalProgressHandle {
  isActive: ComputedRef<boolean>
  start: () => void
  finish: () => void
  cancel: () => void
}

export function useGlobalProgress(): GlobalProgressHandle {
  return {
    isActive: is_active,
    start,
    finish,
    cancel,
  }
}

function start(): void {
  active_count.value += 1
}

function finish(): void {
  if (active_count.value > 0) {
    active_count.value -= 1
  }
}

function cancel(): void {
  active_count.value = 0
}
"#;

const INSTALL_BLOCKING_NAV_TS_BODY: &str = r#"// install-blocking-nav — wires the blocking navigation guard onto a
// vue-router instance. Per SPEC_FRONTEND_ROUTING.md:
//
//   - Each nav grabs an AbortController exposed via getNavAbortSignal()
//     so route component setup() can plumb cancellation into its fetches.
//   - beforeEach: start global progress, mint a fresh AbortController;
//     if a previous nav is still in flight, abort it first.
//   - beforeResolve: race the destination's resolution against the
//     budgetMs timer (default 500ms). On timer expiry, navigate anyway —
//     the destination renders with per-section "still loading" markers.
//     vue-router awaits async setup() inside <Suspense> automatically;
//     this hook ensures we never hang the UI past the budget.
//   - afterEach / onError: finish progress, abort any leftover signal.

import type { NavigationGuardNext, RouteLocationNormalized, Router } from 'vue-router'

import { useGlobalProgress } from '@/composables/global-progress'

export interface InstallBlockingNavOptions {
  budgetMs?: number
}

const DEFAULT_BUDGET_MS = 500

let current_controller: AbortController | null = null

export function getNavAbortSignal(): AbortSignal | null {
  if (current_controller === null) {
    return null
  }
  return current_controller.signal
}

export function installBlockingNav(router: Router, opts?: InstallBlockingNavOptions): void {
  const budget_ms = opts?.budgetMs !== undefined ? opts.budgetMs : DEFAULT_BUDGET_MS
  const progress = useGlobalProgress()

  router.beforeEach((_to: RouteLocationNormalized, _from: RouteLocationNormalized, next: NavigationGuardNext) => {
    if (current_controller !== null) {
      current_controller.abort()
    }
    current_controller = new AbortController()
    progress.start()
    next()
  })

  router.beforeResolve(async (_to: RouteLocationNormalized, _from: RouteLocationNormalized, next: NavigationGuardNext) => {
    const controller = current_controller
    if (controller === null) {
      next()
      return
    }
    let timer: ReturnType<typeof setTimeout> | null = null
    const budget = new Promise<void>((resolve) => {
      timer = setTimeout(() => { resolve() }, budget_ms)
    })
    const aborted = new Promise<void>((resolve) => {
      controller.signal.addEventListener('abort', () => { resolve() }, { once: true })
    })
    await Promise.race([budget, aborted])
    if (timer !== null) {
      clearTimeout(timer)
    }
    next()
  })

  router.afterEach(() => {
    progress.finish()
    if (current_controller !== null) {
      const controller = current_controller
      current_controller = null
      if (!controller.signal.aborted) {
        controller.abort()
      }
    }
  })

  router.onError(() => {
    progress.finish()
    if (current_controller !== null) {
      const controller = current_controller
      current_controller = null
      if (!controller.signal.aborted) {
        controller.abort()
      }
    }
  })
}
"#;

const ROUTER_INDEX_TS_BODY: &str = r#"// router/index.ts — vue-router setup. Routes + per-route guards come
// from `@/generated/router/*` (emitted by `blast gen all`). Blocking-nav
// installs on top from this file. History mode per SPEC_FRONTEND_ROUTING.

import { createRouter, createWebHistory } from 'vue-router'

import { routes } from '@/generated/router/routes'
import { installRouterGuards } from '@/generated/router/install-router-guards'

import { installBlockingNav } from './install-blocking-nav'

export const router = createRouter({
  history: createWebHistory(),
  routes,
})

installRouterGuards(router)
installBlockingNav(router, { budgetMs: 500 })

export default router
"#;

const INDEX_HTML_BODY: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{APP_NAME}}</title>
    <style>
      /*
        Inline styles below are the ONLY exception to the no-inline-style
        rule (Governor InlineStyle / RawColor / HardcodedPx skip this file
        because it lives outside src/). Everything before /src/main.ts has
        loaded must paint without an external stylesheet — hence the px
        values and explicit colors here. Once main.ts mounts, tokens.css
        + base.css take over and these styles become irrelevant.
      */
      html, body { margin: 0; padding: 0; min-height: 100vh; }
      body {
        font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
        background: #0b0d10;
        color: #e6e8eb;
      }
      #app { min-height: 100vh; }
      .pre-mount-shell {
        display: flex;
        align-items: center;
        justify-content: center;
        min-height: 100vh;
      }
      .pre-mount-shell-mark {
        font-size: 14px;
        opacity: 0.6;
        letter-spacing: 0.08em;
        text-transform: uppercase;
      }
      .pre-mount-progress {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        height: 3px;
        overflow: hidden;
        z-index: 9999;
        pointer-events: none;
      }
      .pre-mount-progress-fill {
        width: 40%;
        height: 100%;
        background: #7c3aed;
        animation: pre-mount-slide 1.1s linear infinite;
      }
      @keyframes pre-mount-slide {
        0%   { transform: translateX(-100%); }
        100% { transform: translateX(350%); }
      }
    </style>
  </head>
  <body>
    <div class="pre-mount-progress" aria-hidden="true">
      <div class="pre-mount-progress-fill"></div>
    </div>
    <div id="app">
      <div class="pre-mount-shell">
        <span class="pre-mount-shell-mark">loading</span>
      </div>
    </div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
"#;

const MAIN_TS_BODY: &str = r#"import { createApp } from 'vue'

import App from './App.vue'
import installPrimeVue from './plugins/primevue'
import router from './router'
import './styles/tokens.css'
import './styles/base.css'

const app = createApp(App)
installPrimeVue(app)
app.use(router)
app.mount('#app')
"#;

pub struct ScaffoldOutcome {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

/// Emit all FE runtime scaffold files. Each file is `write_if_absent` —
/// existing user customizations survive `blast gen all`.
///
/// `app_name` is substituted into the index.html `<title>`.
pub fn run(project_root: &Path, app_name: &str) -> BlastResult<ScaffoldOutcome> {
    let mut written = Vec::new();
    let mut skipped = Vec::new();

    let static_targets: [(&str, &str); 10] = [
        (PAGE_SHELL_RELATIVE, PAGE_SHELL_VUE),
        (GLOBAL_PROGRESS_BAR_RELATIVE, GLOBAL_PROGRESS_BAR_VUE),
        (GLOBAL_PROGRESS_TS_RELATIVE, GLOBAL_PROGRESS_TS),
        (INSTALL_BLOCKING_NAV_RELATIVE, INSTALL_BLOCKING_NAV_TS),
        (ROUTER_INDEX_RELATIVE, ROUTER_INDEX_TS),
        (MAIN_TS_RELATIVE, MAIN_TS),
        (URL_TS_RELATIVE, URL_TS),
        (DIALOG_TS_RELATIVE, DIALOG_TS),
        (DRAWER_TS_RELATIVE, DRAWER_TS),
        (CHANNEL_TS_RELATIVE, CHANNEL_TS),
    ];
    for (rel, body) in static_targets.iter() {
        let target = project_root.join(rel);
        match write_if_absent(&target, body)? {
            true => written.push(target),
            false => skipped.push(target),
        }
    }

    // index.html is templated — substitute app name at emission time.
    let index_html_target = project_root.join(INDEX_HTML_RELATIVE);
    let index_html_body = INDEX_HTML_TEMPLATE.replace(APP_NAME_TOKEN, app_name);
    match write_if_absent(&index_html_target, &index_html_body)? {
        true => written.push(index_html_target),
        false => skipped.push(index_html_target),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir_with_app() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let project_root = dir.path().to_path_buf();
        (dir, project_root)
    }

    #[test]
    fn page_shell_constant_is_nonempty_and_has_layout_prop() {
        assert!(!PAGE_SHELL_VUE.is_empty());
        assert!(PAGE_SHELL_VUE.contains("PageLayout"));
        assert!(PAGE_SHELL_VUE.contains("'cards' | 'split' | 'table' | 'bleed' | 'tabbed'"));
        assert!(PAGE_SHELL_VUE.contains("data-layout='cards'"));
        assert!(PAGE_SHELL_VUE.contains("data-layout='split'"));
        assert!(PAGE_SHELL_VUE.contains("data-layout='table'"));
        assert!(PAGE_SHELL_VUE.contains("data-layout='bleed'"));
        assert!(PAGE_SHELL_VUE.contains("data-layout='tabbed'"));
        assert!(PAGE_SHELL_VUE.contains("@layer app"));
    }

    #[test]
    fn page_shell_has_no_hex_colors() {
        // Crude check: no '#' followed by hex digits.
        for line in PAGE_SHELL_VUE.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }
            for (idx, _) in line.match_indices('#') {
                let after = &line[idx + 1..];
                let next = after.chars().next();
                match next {
                    Some(c) if c.is_ascii_hexdigit() => {
                        panic!("PageShell contains hex color literal in line: {}", line);
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn global_progress_bar_constant_is_nonempty_and_uses_tokens() {
        assert!(!GLOBAL_PROGRESS_BAR_VUE.is_empty());
        assert!(GLOBAL_PROGRESS_BAR_VUE.contains("useGlobalProgress"));
        assert!(GLOBAL_PROGRESS_BAR_VUE.contains("var(--p-primary-color)"));
        assert!(GLOBAL_PROGRESS_BAR_VUE.contains("var(--app-z-overlay)"));
        assert!(GLOBAL_PROGRESS_BAR_VUE.contains("@keyframes global-progress-bar-slide"));
        assert!(GLOBAL_PROGRESS_BAR_VUE.contains("aria-live"));
    }

    #[test]
    fn global_progress_ts_exports_handle() {
        assert!(GLOBAL_PROGRESS_TS.contains("export function useGlobalProgress"));
        assert!(GLOBAL_PROGRESS_TS.contains("isActive"));
        assert!(GLOBAL_PROGRESS_TS.contains("start"));
        assert!(GLOBAL_PROGRESS_TS.contains("finish"));
        assert!(GLOBAL_PROGRESS_TS.contains("cancel"));
        // No `any` types.
        assert!(!GLOBAL_PROGRESS_TS.contains(": any"));
        assert!(!GLOBAL_PROGRESS_TS.contains("as any"));
    }

    #[test]
    fn install_blocking_nav_exports_installer_and_signal_helper() {
        assert!(INSTALL_BLOCKING_NAV_TS.contains("export function installBlockingNav"));
        assert!(INSTALL_BLOCKING_NAV_TS.contains("export function getNavAbortSignal"));
        assert!(INSTALL_BLOCKING_NAV_TS.contains("AbortController"));
        assert!(INSTALL_BLOCKING_NAV_TS.contains("budgetMs"));
        assert!(INSTALL_BLOCKING_NAV_TS.contains("router.beforeEach"));
        assert!(INSTALL_BLOCKING_NAV_TS.contains("router.beforeResolve"));
        assert!(INSTALL_BLOCKING_NAV_TS.contains("router.afterEach"));
        assert!(INSTALL_BLOCKING_NAV_TS.contains("router.onError"));
        assert!(!INSTALL_BLOCKING_NAV_TS.contains("console.log"));
    }

    #[test]
    fn router_index_imports_generated_and_local() {
        assert!(ROUTER_INDEX_TS.contains("createRouter"));
        assert!(ROUTER_INDEX_TS.contains("createWebHistory"));
        assert!(ROUTER_INDEX_TS.contains("@/generated/router/routes"));
        assert!(ROUTER_INDEX_TS.contains("@/generated/router/install-router-guards"));
        assert!(ROUTER_INDEX_TS.contains("./install-blocking-nav"));
        assert!(ROUTER_INDEX_TS.contains("budgetMs: 500"));
    }

    #[test]
    fn index_html_template_has_substitution_token() {
        assert!(INDEX_HTML_TEMPLATE.contains("{{APP_NAME}}"));
        assert!(INDEX_HTML_TEMPLATE.contains("pre-mount-progress"));
        assert!(INDEX_HTML_TEMPLATE.contains("pre-mount-shell"));
        assert!(INDEX_HTML_TEMPLATE.contains("/src/main.ts"));
    }

    #[test]
    fn main_ts_installs_router_and_primevue() {
        assert!(MAIN_TS.contains("import router from './router'"));
        assert!(MAIN_TS.contains("app.use(router)"));
        assert!(MAIN_TS.contains("installPrimeVue(app)"));
        assert!(MAIN_TS.contains("./styles/tokens.css"));
        assert!(MAIN_TS.contains("./styles/base.css"));
    }

    #[test]
    fn run_writes_all_files_in_empty_project() {
        let (_dir, root) = tempdir_with_app();
        let outcome = run(&root, "acme").expect("run");
        assert_eq!(outcome.written.len(), 11);
        assert_eq!(outcome.skipped.len(), 0);

        assert!(root.join(PAGE_SHELL_RELATIVE).is_file());
        assert!(root.join(GLOBAL_PROGRESS_BAR_RELATIVE).is_file());
        assert!(root.join(GLOBAL_PROGRESS_TS_RELATIVE).is_file());
        assert!(root.join(INSTALL_BLOCKING_NAV_RELATIVE).is_file());
        assert!(root.join(ROUTER_INDEX_RELATIVE).is_file());
        assert!(root.join(MAIN_TS_RELATIVE).is_file());
        assert!(root.join(INDEX_HTML_RELATIVE).is_file());
        assert!(root.join(URL_TS_RELATIVE).is_file());
        assert!(root.join(DIALOG_TS_RELATIVE).is_file());
        assert!(root.join(DRAWER_TS_RELATIVE).is_file());
        assert!(root.join(CHANNEL_TS_RELATIVE).is_file());
    }

    #[test]
    fn run_substitutes_app_name_into_index_html() {
        let (_dir, root) = tempdir_with_app();
        run(&root, "acme-corp").expect("run");
        let body = fs::read_to_string(root.join(INDEX_HTML_RELATIVE)).expect("read");
        assert!(body.contains("<title>acme-corp</title>"));
        assert!(!body.contains("{{APP_NAME}}"));
    }

    #[test]
    fn run_is_idempotent_and_skips_existing_files() {
        let (_dir, root) = tempdir_with_app();
        let first = run(&root, "acme").expect("first");
        assert_eq!(first.written.len(), 11);

        // Mutate one file to verify it's not stomped.
        let custom_marker = "/* user-customized */\n";
        let page_shell_path = root.join(PAGE_SHELL_RELATIVE);
        fs::write(&page_shell_path, custom_marker).expect("overwrite");

        let second = run(&root, "acme").expect("second");
        assert_eq!(second.written.len(), 0);
        assert_eq!(second.skipped.len(), 11);

        let after = fs::read_to_string(&page_shell_path).expect("read");
        assert_eq!(after, custom_marker);
    }

}
