import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'

import { useAuth } from '@/composables/auth'
import { installRouterGuards } from '@/generated/router/install-router-guards'
import { routes as generatedRoutes } from '@/generated/router/routes'

import { installBlockingNav } from './install-blocking-nav'

const userRoutes: RouteRecordRaw[] = [
  { path: '/', name: 'welcome', component: () => import('@/pages/WelcomePage.vue') },
  { path: '/login', name: 'auth.login', component: () => import('@/pages/LoginPage.vue') },
  { path: '/register', name: 'auth.register', component: () => import('@/pages/RegisterPage.vue') },
  { path: '/dashboard', name: 'dashboard', component: () => import('@/pages/DashboardPage.vue'), meta: { requires_auth: true } },
  { path: '/profile', name: 'profile', component: () => import('@/pages/ProfilePage.vue'), meta: { requires_auth: true } },
  { path: '/:pathMatch(.*)*', name: 'not-found', component: () => import('@/pages/NotFoundPage.vue') },
]

export const router = createRouter({
  history: createWebHistory(),
  routes: [...generatedRoutes, ...userRoutes],
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
