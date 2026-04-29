<script setup lang="ts">
import { ref, computed } from 'vue'
import InputText from 'primevue/inputtext'
import Password from 'primevue/password'
import Button from 'primevue/button'
import { validateLoginInput } from '@/composables/auth_validators'
import AuthFormField from '@/components/AuthFormField.vue'

const emit = defineEmits<{
  (e: 'submit', payload: { email: string; password: string }): void
}>()

defineProps<{
  loading: boolean
  serverError: string | null
}>()

const email = ref('')
const password = ref('')

const field_errors = computed(() => validateLoginInput({ email: email.value, password: password.value }))
const email_invalid = computed(() => email.value.length > 0 && field_errors.value.email !== undefined)
const password_invalid = computed(() => password.value.length > 0 && field_errors.value.password !== undefined)
const form_valid = computed(() => Object.keys(field_errors.value).length === 0)

function on_submit(): void {
  if (!form_valid.value) return
  emit('submit', { email: email.value, password: password.value })
}
</script>

<template>
  <form class="login-form" novalidate @submit.prevent="on_submit">
    <AuthFormField
      input-id="login-email"
      label="Email"
      :invalid="email_invalid"
      :error="field_errors.email"
    >
      <InputText
        id="login-email"
        v-model="email"
        type="email"
        autocomplete="email"
        placeholder="you@example.com"
        :invalid="email_invalid"
        class="login-input"
      />
    </AuthFormField>

    <AuthFormField
      input-id="login-password"
      label="Password"
      :invalid="password_invalid"
      :error="field_errors.password"
    >
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
    </AuthFormField>

    <div v-if="serverError !== null" class="login-server-error" role="alert">
      {{ serverError }}
    </div>

    <Button
      type="submit"
      label="Sign in"
      :loading="loading"
      :disabled="!form_valid || loading"
      class="login-submit"
    />
  </form>
</template>

<style scoped>
@layer app {
  .login-form {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-md);
  }

  .login-input {
    width: 100%;
  }

  .login-password-wrap {
    width: 100%;
  }

  .login-server-error {
    background: var(--p-red-50, color-mix(in srgb, var(--p-danger-color) 10%, transparent));
    color: var(--p-red-700, var(--p-danger-color));
    border: 0.0625rem solid var(--p-red-200, var(--p-danger-color));
    border-radius: var(--p-border-radius);
    padding: var(--app-space-sm) var(--app-space-md);
    font-size: var(--app-fs-sm);
  }

  .login-submit {
    width: 100%;
    margin-top: var(--app-space-xs);
  }
}
</style>
