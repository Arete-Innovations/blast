<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import { useAuth } from '@/composables/auth'
import { validateRegisterInput } from '@/composables/auth_validators'

const router = useRouter()
const { register } = useAuth()

const email = ref('')
const password = ref('')
const confirm_password = ref('')
const loading = ref(false)
const server_error = ref<string | null>(null)

const field_errors = computed<Record<string, string>>(() => validateRegisterInput({ email: email.value, password: password.value, confirm_password: confirm_password.value }) ?? {})
const email_invalid = computed<boolean>(() => email.value.length > 0 && field_errors.value.email !== undefined)
const password_invalid = computed<boolean>(() => password.value.length > 0 && field_errors.value.password !== undefined)
const confirm_invalid = computed<boolean>(() => confirm_password.value.length > 0 && field_errors.value.confirm_password !== undefined)
const form_valid = computed<boolean>(() => Object.keys(field_errors.value).length === 0)

async function handle_submit(): Promise<void> {
  if (!form_valid.value || loading.value) return
  loading.value = true
  server_error.value = null
  const result = await register(email.value, password.value)
  loading.value = false
  if (!result.ok) {
    server_error.value = result.error ?? 'Registration failed'
    return
  }
  await router.push({ name: 'dashboard' })
}
</script>

<template>
  <PageShell layout="bleed">
    <div class="register-wrap">
      <div class="register-card">
        <h1 class="register-title">Create an account</h1>

        <form class="register-form" novalidate @submit.prevent="handle_submit">
          <div class="register-field">
            <label for="register-email" class="register-label">Email</label>
            <InputText
              id="register-email"
              v-model="email"
              type="email"
              autocomplete="email"
              placeholder="you@example.com"
              :invalid="email_invalid"
              class="register-input"
            />
            <span v-if="email_invalid" class="register-hint register-hint--error">
              {{ field_errors.email }}
            </span>
          </div>

          <div class="register-field">
            <label for="register-password" class="register-label">Password</label>
            <Password
              id="register-password"
              v-model="password"
              :feedback="true"
              toggle-mask
              autocomplete="new-password"
              placeholder="At least 8 characters"
              :invalid="password_invalid"
              class="register-input"
            />
            <span v-if="password_invalid" class="register-hint register-hint--error">
              {{ field_errors.password }}
            </span>
          </div>

          <div class="register-field">
            <label for="register-confirm" class="register-label">Confirm password</label>
            <Password
              id="register-confirm"
              v-model="confirm_password"
              :feedback="false"
              toggle-mask
              autocomplete="new-password"
              placeholder="Repeat your password"
              :invalid="confirm_invalid"
              class="register-input"
            />
            <span v-if="confirm_invalid" class="register-hint register-hint--error">
              {{ field_errors.confirm_password }}
            </span>
          </div>

          <div v-if="server_error" class="register-server-error" role="alert">
            {{ server_error }}
          </div>

          <Button
            type="submit"
            label="Create account"
            :loading="loading"
            :disabled="!form_valid || loading"
            class="register-submit"
          />
        </form>

        <p class="register-footer">
          Already have an account?
          <router-link :to="{ name: 'auth.login' }" class="register-link">Sign in</router-link>
        </p>
      </div>
    </div>
  </PageShell>
</template>

<style scoped>
@layer app {
  .register-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: var(--app-space-md);
    background: var(--p-content-background);
  }

  .register-card {
    width: 100%;
    max-width: 24rem;
    background: var(--p-surface-card);
    border: 0.0625rem solid var(--p-content-border-color);
    border-radius: var(--app-radius-md);
    padding: var(--app-space-xl);
    display: flex;
    flex-direction: column;
    gap: var(--app-space-lg);
  }

  .register-title {
    margin: 0;
    font-size: var(--app-text-xl);
    font-weight: var(--app-weight-semibold);
    color: var(--p-text-color);
    text-align: center;
  }

  .register-form {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-md);
  }

  .register-field {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-xs);
  }

  .register-label {
    font-size: var(--app-text-sm);
    font-weight: var(--app-weight-medium);
    color: var(--p-text-color);
  }

  .register-input {
    width: 100%;
  }

  .register-hint {
    font-size: var(--app-text-xs);
  }

  .register-hint--error {
    color: var(--p-red-500);
  }

  .register-server-error {
    font-size: var(--app-text-sm);
    color: var(--p-red-500);
    background: var(--p-red-50);
    border: 0.0625rem solid var(--p-red-200);
    border-radius: var(--app-radius-sm);
    padding: var(--app-space-sm) var(--app-space-md);
  }

  .register-submit {
    width: 100%;
    margin-top: var(--app-space-xs);
  }

  .register-footer {
    margin: 0;
    text-align: center;
    font-size: var(--app-text-sm);
    color: var(--p-text-muted-color);
  }

  .register-link {
    color: var(--p-primary-color);
    font-weight: var(--app-weight-medium);
  }

  .register-link:hover {
    text-decoration: underline;
  }
}
</style>
