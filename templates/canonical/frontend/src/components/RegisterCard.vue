<script setup lang="ts">
import { ref, computed } from 'vue'
import InputText from 'primevue/inputtext'
import Password from 'primevue/password'
import Button from 'primevue/button'
import { validateRegisterInput } from '@/composables/auth_validators'
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
const confirm_password = ref('')

const field_errors = computed(() => validateRegisterInput({
  email: email.value,
  password: password.value,
  confirm_password: confirm_password.value,
}))
const email_invalid = computed(() => email.value.length > 0 && field_errors.value.email !== undefined)
const password_invalid = computed(() => password.value.length > 0 && field_errors.value.password !== undefined)
const confirm_invalid = computed(() => confirm_password.value.length > 0 && field_errors.value.confirm_password !== undefined)
const form_valid = computed(() => Object.keys(field_errors.value).length === 0)

function on_submit(): void {
  if (!form_valid.value) return
  emit('submit', { email: email.value, password: password.value })
}
</script>

<template>
  <form class="register-form" novalidate @submit.prevent="on_submit">
    <AuthFormField
      input-id="register-email"
      label="Email"
      :invalid="email_invalid"
      :error="field_errors.email"
    >
      <InputText
        id="register-email"
        v-model="email"
        type="email"
        autocomplete="email"
        placeholder="you@example.com"
        :invalid="email_invalid"
        class="register-input"
      />
    </AuthFormField>

    <AuthFormField
      input-id="register-password"
      label="Password"
      :invalid="password_invalid"
      :error="field_errors.password"
    >
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
    </AuthFormField>

    <AuthFormField
      input-id="register-confirm"
      label="Confirm password"
      :invalid="confirm_invalid"
      :error="field_errors.confirm_password"
    >
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
    </AuthFormField>

    <div v-if="serverError !== null" class="register-server-error" role="alert">
      {{ serverError }}
    </div>

    <Button
      type="submit"
      label="Create account"
      :loading="loading"
      :disabled="!form_valid || loading"
      class="register-submit"
    />
  </form>
</template>

<style scoped>
@layer app {
  .register-form {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-md);
  }

  .register-input {
    width: 100%;
  }

  .register-server-error {
    font-size: var(--app-fs-sm);
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
}
</style>
