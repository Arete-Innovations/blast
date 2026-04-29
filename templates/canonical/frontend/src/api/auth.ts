export interface AuthUser {
  id: number
  email: string
  role: string
}

export interface AuthLoginResponse {
  token: string
  user?: AuthUser
}

export interface AuthRegisterResponse {
  token: string
  user?: AuthUser
}

export interface AuthErrorResponse {
  error: string
}

export type AuthResult<T> = { ok: true; data: T } | { ok: false; error: string }

function json_headers(token: string | null): HeadersInit {
  if (token === null) {
    return { 'Content-Type': 'application/json' }
  }
  return {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${token}`,
  }
}

async function read_error_message(res: Response, fallback: string): Promise<string> {
  const data: unknown = await res.json().catch(() => null)
  if (
    data !== null &&
    typeof data === 'object' &&
    'error' in data &&
    typeof (data as { error: unknown }).error === 'string'
  ) {
    return (data as { error: string }).error
  }
  return fallback
}

function is_login_response(data: unknown): data is AuthLoginResponse {
  return (
    data !== null &&
    typeof data === 'object' &&
    'token' in data &&
    typeof (data as { token: unknown }).token === 'string'
  )
}

function is_auth_user(data: unknown): data is AuthUser {
  return (
    data !== null &&
    typeof data === 'object' &&
    'id' in data &&
    'email' in data &&
    'role' in data
  )
}

export async function login(email: string, password: string): Promise<AuthResult<AuthLoginResponse>> {
  const res = await fetch('/api/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })
  if (!res.ok) {
    const message = await read_error_message(res, 'Login failed')
    return { ok: false, error: message }
  }
  const data: unknown = await res.json()
  if (!is_login_response(data)) {
    return { ok: false, error: 'Unexpected response from server' }
  }
  return { ok: true, data }
}

export async function register(email: string, password: string): Promise<AuthResult<AuthRegisterResponse>> {
  const res = await fetch('/api/auth/register', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })
  if (!res.ok) {
    const message = await read_error_message(res, 'Registration failed')
    return { ok: false, error: message }
  }
  const data: unknown = await res.json()
  if (!is_login_response(data)) {
    return { ok: false, error: 'Unexpected response from server' }
  }
  return { ok: true, data }
}

export async function logout(token: string): Promise<void> {
  await fetch('/api/auth/logout', {
    method: 'POST',
    headers: json_headers(token),
  }).catch(() => undefined)
}

export type FetchMeOutcome =
  | { kind: 'user'; user: AuthUser }
  | { kind: 'unauthenticated' }
  | { kind: 'transient' }

export async function fetch_me(token: string): Promise<FetchMeOutcome> {
  const res = await fetch('/api/auth/me', {
    headers: json_headers(token),
  })
  if (res.status === 401) {
    return { kind: 'unauthenticated' }
  }
  if (!res.ok) {
    return { kind: 'transient' }
  }
  const data: unknown = await res.json().catch(() => null)
  if (is_auth_user(data)) {
    return { kind: 'user', user: data }
  }
  return { kind: 'transient' }
}
