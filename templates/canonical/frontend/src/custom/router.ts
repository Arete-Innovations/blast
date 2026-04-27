import type { RouteRecordRaw } from 'vue-router'

export const customRoutes: RouteRecordRaw[] = [
  { path: '/', name: 'welcome', component: () => import('@/custom/pages/WelcomePage.vue') },
  { path: '/login', name: 'auth.login', component: () => import('@/custom/pages/LoginPage.vue') },
  { path: '/register', name: 'auth.register', component: () => import('@/custom/pages/RegisterPage.vue') },
  { path: '/dashboard', name: 'dashboard', component: () => import('@/custom/pages/DashboardPage.vue'), meta: { requires_auth: true } },
  { path: '/profile', name: 'profile', component: () => import('@/custom/pages/ProfilePage.vue'), meta: { requires_auth: true } },
  { path: '/:pathMatch(.*)*', name: 'not-found', component: () => import('@/custom/pages/NotFoundPage.vue') },
]
