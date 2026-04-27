import { createRouter, createWebHistory } from 'vue-router'

import { routes as generatedRoutes } from '@/generated/router/routes'
import { installRouterGuards } from '@/generated/router/install-router-guards'
import { customRoutes } from '@/custom/router'
import { useAuth } from '@/composables/auth'

import { installBlockingNav } from './install-blocking-nav'

export const router = createRouter({
  history: createWebHistory(),
  routes: [...generatedRoutes, ...customRoutes],
})

router.beforeEach((to) => {
  const requires_auth = to.meta.requires_auth === true
  if (requires_auth && !useAuth().is_authenticated.value) {
    return { name: 'auth.login', query: { redirect: to.fullPath } }
  }
})

installRouterGuards(router)
installBlockingNav(router, { budgetMs: 500 })

export default router
