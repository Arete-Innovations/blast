<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import { useAuth } from '@/composables/auth'
import { validateLoginInput } from '@/composables/auth_validators'

const router = useRouter()
const auth = useAuth()

const email = ref('')
const password = ref('')
const loading = ref(false)
const server_error = ref<string | null>(null)

const field_errors = computed<Record<string, string>>(() => validateLoginInput({ email: email.value, password: password.value }) ?? {})
const email_invalid = computed<boolean>(() => email.value.length > 0 && field_errors.value.email !== undefined)
const password_invalid = computed<boolean>(() => password.value.length > 0 && field_errors.value.password !== undefined)
const form_valid = computed<boolean>(() => Object.keys(field_errors.value).length === 0)

async function handle_submit(): Promise<void> {
  if (!form_valid.value || loading.value) return
  server_error.value = null
  loading.value = true
  try {
    const result = await auth.login(email.value, password.value)
    if (result.ok) {
      await router.push({ name: 'dashboard' })
    } else {
      server_error.value = result.error ?? 'Login failed'
    }
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <PageShell layout="bleed">
    <div class="login-page">
      <div class="login-card">
        <h1 class="login-title">Sign in</h1>

        <form class="login-form" novalidate @submit.prevent="handle_submit">
          <div class="login-field">
            <label for="login-email" class="login-label">Email</label>
            <InputText
              id="login-email"
              v-model="email"
              type="email"
              autocomplete="email"
              placeholder="you@example.com"
              :invalid="email_invalid"
              class="login-input"
            />
            <small v-if="email_invalid" class="login-field-error">
              {{ field_errors.email }}
            </small>
          </div>

          <div class="login-field">
            <label for="login-password" class="login-label">Password</label>
            <Password
              id="login-password"
              v-model="password"
              :feedback="false"
              toggle-mask
              autocomplete="current-password"
              placeholder="Password"
              :invalid="password_invalid"
              input-class="login-input"
              class="login-password-wrap"
            />
            <small v-if="password_invalid" class="login-field-error">
              {{ field_errors.password }}
            </small>
          </div>

          <div v-if="server_error !== null" class="login-server-error" role="alert">
            {{ server_error }}
          </div>

          <Button
            type="submit"
            label="Sign in"
            :loading="loading"
            :disabled="!form_valid || loading"
            class="login-submit"
          />
        </form>

        <p class="login-footer">
          Don't have an account?
          <router-link :to="{ name: 'auth.register' }" class="login-register-link">Create one</router-link>
        </p>
      </div>
    </div>
  </PageShell>
</template>

<style scoped>
@layer app {
  .login-page {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: var(--app-space-md);
    background: var(--p-content-background);
  }

  .login-card {
    width: 100%;
    max-width: var(--app-form-max-width, 26rem);
    background: var(--p-surface-0);
    border: 0.0625rem solid var(--p-content-border-color);
    border-radius: var(--p-border-radius-lg, var(--p-border-radius));
    padding: var(--app-space-xl, var(--app-space-lg));
    display: flex;
    flex-direction: column;
    gap: var(--app-space-lg);
  }

  .login-title {
    margin: 0;
    font-size: var(--app-text-2xl, 1.5rem);
    font-weight: 700;
    color: var(--p-text-color);
    text-align: center;
  }

  .login-form {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-md);
  }

  .login-field {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-xs, calc(var(--app-space-sm) * 0.5));
  }

  .login-label {
    font-size: var(--app-text-sm, 0.875rem);
    font-weight: 600;
    color: var(--p-text-color);
  }

  .login-input {
    width: 100%;
  }

  .login-password-wrap {
    width: 100%;
  }

  .login-field-error {
    color: var(--p-red-500, var(--p-danger-color));
    font-size: var(--app-text-xs, 0.75rem);
  }

  .login-server-error {
    background: var(--p-red-50, color-mix(in srgb, var(--p-danger-color) 10%, transparent));
    color: var(--p-red-700, var(--p-danger-color));
    border: 0.0625rem solid var(--p-red-200, var(--p-danger-color));
    border-radius: var(--p-border-radius);
    padding: var(--app-space-sm) var(--app-space-md);
    font-size: var(--app-text-sm, 0.875rem);
  }

  .login-submit {
    width: 100%;
    margin-top: var(--app-space-xs, calc(var(--app-space-sm) * 0.5));
  }

  .login-footer {
    margin: 0;
    text-align: center;
    font-size: var(--app-text-sm, 0.875rem);
    color: var(--p-text-muted-color);
  }

  .login-register-link {
    color: var(--p-primary-color);
    font-weight: 600;
    text-decoration: underline;
  }

  .login-register-link:hover {
    opacity: 0.8;
  }
}
</style>
