// router/index.ts — vue-router setup. Routes + per-route guards come
// from `@/generated/router/*` (emitted by `blast gen all`). Blocking-nav
// installs on top from this file. History mode per SPEC_FRONTEND_ROUTING.
//
// Custom routes live in `@/custom/router` and are appended after the
// generated routes. Add app-specific landing pages, auth screens, etc.
// there without touching this framework file.

import { createRouter, createWebHistory } from 'vue-router'

import { routes as generatedRoutes } from '@/generated/router/routes'
import { installRouterGuards } from '@/generated/router/install-router-guards'
import { customRoutes } from '@/custom/router'

import { installBlockingNav } from './install-blocking-nav'

export const router = createRouter({
  history: createWebHistory(),
  routes: [...generatedRoutes, ...customRoutes],
})

installRouterGuards(router)
installBlockingNav(router, { budgetMs: 500 })

export default router
