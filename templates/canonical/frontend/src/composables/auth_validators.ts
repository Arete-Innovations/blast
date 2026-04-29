export type FieldErrors = Record<string, string>

export interface LoginInput {
  email: string
  password: string
}

export interface RegisterInput {
  email: string
  password: string
  confirm_password: string
}

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

export function validateLoginInput(input: LoginInput): FieldErrors | null {
  const errors: FieldErrors = {}
  if (typeof input.email !== 'string' || input.email.length === 0) {
    errors.email = 'required'
  } else if (!EMAIL_RE.test(input.email)) {
    errors.email = 'must be a valid email'
  } else if ([...input.email].length > 254) {
    errors.email = 'must be at most 254 characters'
  }
  if (typeof input.password !== 'string' || input.password.length === 0) {
    errors.password = 'required'
  }
  return Object.keys(errors).length === 0 ? null : errors
}

export function validateRegisterInput(input: RegisterInput): FieldErrors | null {
  const errors: FieldErrors = {}
  if (typeof input.email !== 'string' || input.email.length === 0) {
    errors.email = 'required'
  } else if (!EMAIL_RE.test(input.email)) {
    errors.email = 'must be a valid email'
  } else if ([...input.email].length > 254) {
    errors.email = 'must be at most 254 characters'
  }
  if (typeof input.password !== 'string' || input.password.length === 0) {
    errors.password = 'required'
  } else if ([...input.password].length < 8) {
    errors.password = 'must be at least 8 characters'
  }
  if (typeof input.confirm_password !== 'string' || input.confirm_password.length === 0) {
    errors.confirm_password = 'required'
  } else if (input.confirm_password !== input.password) {
    errors.confirm_password = 'passwords do not match'
  }
  return Object.keys(errors).length === 0 ? null : errors
}
