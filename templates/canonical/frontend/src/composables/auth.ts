import { computed, ref } from 'vue'
import type { ComputedRef, Ref } from 'vue'
import * as auth_api from '@/api/auth'
import type { AuthUser } from '@/api/auth'

const AUTH_TOKEN_KEY = 'auth_token'

export type { AuthUser } from '@/api/auth'

export interface UseAuth {
  current_user: Ref<AuthUser | null>
  is_authenticated: ComputedRef<boolean>
  login: (email: string, password: string) => Promise<{ ok: boolean; error?: string }>
  register: (email: string, password: string) => Promise<{ ok: boolean; error?: string }>
  logout: () => Promise<void>
  refresh: () => Promise<void>
}

const current_user = ref<AuthUser | null>(null)
const token = ref<string | null>(localStorage.getItem(AUTH_TOKEN_KEY))

function store_token(raw: string): void {
  token.value = raw
  localStorage.setItem(AUTH_TOKEN_KEY, raw)
}

function clear_token(): void {
  token.value = null
  localStorage.removeItem(AUTH_TOKEN_KEY)
}

async function login(email: string, password: string): Promise<{ ok: boolean; error?: string }> {
  const result = await auth_api.login(email, password)
  if (!result.ok) {
    return { ok: false, error: result.error }
  }
  store_token(result.data.token)
  if (result.data.user !== undefined) {
    current_user.value = result.data.user
  } else {
    await refresh()
  }
  return { ok: true }
}

async function register(email: string, password: string): Promise<{ ok: boolean; error?: string }> {
  const result = await auth_api.register(email, password)
  if (!result.ok) {
    return { ok: false, error: result.error }
  }
  store_token(result.data.token)
  if (result.data.user !== undefined) {
    current_user.value = result.data.user
  } else {
    await refresh()
  }
  return { ok: true }
}

async function logout(): Promise<void> {
  if (token.value !== null) {
    await auth_api.logout(token.value)
  }
  current_user.value = null
  clear_token()
}

async function refresh(): Promise<void> {
  if (token.value === null) {
    current_user.value = null
    return
  }
  const outcome = await auth_api.fetch_me(token.value)
  if (outcome.kind === 'user') {
    current_user.value = outcome.user
    return
  }
  if (outcome.kind === 'unauthenticated') {
    current_user.value = null
    clear_token()
  }
}

export function useAuth(): UseAuth {
  return {
    current_user,
    is_authenticated: computed<boolean>(() => current_user.value !== null),
    login,
    register,
    logout,
    refresh,
  }
}
