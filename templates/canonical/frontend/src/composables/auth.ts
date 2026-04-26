// auth — singleton auth composable. Wraps /api/auth/{register,login,logout,me}.
// This is the ONLY place in the app that reads/writes localStorage for auth.
// The Governor LocalStorageOutsidePersistence rule exempts this file by path.
//
// Module-level refs are intentional singletons — every call to useAuth()
// returns the same reactive state. Token is loaded from localStorage once
// at module init; subsequent reads/writes are in-memory.

import { computed, ref } from 'vue'
import type { ComputedRef, Ref } from 'vue'

const AUTH_TOKEN_KEY = 'auth_token'

export interface AuthUser {
  id: number
  email: string
  role: string
}

export interface UseAuth {
  current_user: Ref<AuthUser | null>
  is_authenticated: ComputedRef<boolean>
  login: (email: string, password: string) => Promise<{ ok: boolean; error?: string }>
  register: (email: string, password: string) => Promise<{ ok: boolean; error?: string }>
  logout: () => Promise<void>
  refresh: () => Promise<void>
}

// Module-level singletons — shared across all useAuth() callers.
const current_user = ref<AuthUser | null>(null)
const token = ref<string | null>(localStorage.getItem(AUTH_TOKEN_KEY)) // allow: auth persistence layer

function auth_headers(): HeadersInit {
  if (token.value === null) {
    return { 'Content-Type': 'application/json' }
  }
  return {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${token.value}`,
  }
}

function store_token(raw: string): void {
  token.value = raw
  localStorage.setItem(AUTH_TOKEN_KEY, raw) // allow: auth persistence layer
}

function clear_token(): void {
  token.value = null
  localStorage.removeItem(AUTH_TOKEN_KEY) // allow: auth persistence layer
}

async function login(email: string, password: string): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch('/api/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })

  if (!res.ok) {
    const data: unknown = await res.json().catch(() => null)
    const message =
      data !== null &&
      typeof data === 'object' &&
      'error' in data &&
      typeof (data as { error: unknown }).error === 'string'
        ? (data as { error: string }).error
        : 'Login failed'
    return { ok: false, error: message }
  }

  const data: unknown = await res.json()
  if (
    data === null ||
    typeof data !== 'object' ||
    !('token' in data) ||
    typeof (data as { token: unknown }).token !== 'string'
  ) {
    return { ok: false, error: 'Unexpected response from server' }
  }

  store_token((data as { token: string }).token)

  if (
    'user' in data &&
    data !== null &&
    typeof (data as { user: unknown }).user === 'object' &&
    (data as { user: unknown }).user !== null
  ) {
    current_user.value = (data as { user: AuthUser }).user
  } else {
    await refresh()
  }

  return { ok: true }
}

async function register(email: string, password: string): Promise<{ ok: boolean; error?: string }> {
  const res = await fetch('/api/auth/register', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })

  if (!res.ok) {
    const data: unknown = await res.json().catch(() => null)
    const message =
      data !== null &&
      typeof data === 'object' &&
      'error' in data &&
      typeof (data as { error: unknown }).error === 'string'
        ? (data as { error: string }).error
        : 'Registration failed'
    return { ok: false, error: message }
  }

  const data: unknown = await res.json()
  if (
    data === null ||
    typeof data !== 'object' ||
    !('token' in data) ||
    typeof (data as { token: unknown }).token !== 'string'
  ) {
    return { ok: false, error: 'Unexpected response from server' }
  }

  store_token((data as { token: string }).token)

  if (
    'user' in data &&
    data !== null &&
    typeof (data as { user: unknown }).user === 'object' &&
    (data as { user: unknown }).user !== null
  ) {
    current_user.value = (data as { user: AuthUser }).user
  } else {
    await refresh()
  }

  return { ok: true }
}

async function logout(): Promise<void> {
  if (token.value !== null) {
    await fetch('/api/auth/logout', {
      method: 'POST',
      headers: auth_headers(),
    }).catch(() => undefined)
  }
  current_user.value = null
  clear_token()
}

async function refresh(): Promise<void> {
  if (token.value === null) {
    current_user.value = null
    return
  }

  const res = await fetch('/api/auth/me', {
    headers: auth_headers(),
  })

  if (res.status === 401) {
    current_user.value = null
    clear_token()
    return
  }

  if (!res.ok) {
    return
  }

  const data: unknown = await res.json().catch(() => null)
  if (
    data !== null &&
    typeof data === 'object' &&
    'id' in data &&
    'email' in data &&
    'role' in data
  ) {
    current_user.value = data as AuthUser
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
