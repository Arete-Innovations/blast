// install-blocking-nav — wires the blocking navigation guard onto a
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
