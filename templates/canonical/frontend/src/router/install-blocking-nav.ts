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
