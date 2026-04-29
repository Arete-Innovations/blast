<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import AuthShell from '@/components/AuthShell.vue'
import RegisterCard from '@/components/RegisterCard.vue'
import { useAuth } from '@/composables/auth'

const router = useRouter()
const { register } = useAuth()

const loading = ref(false)
const server_error = ref<string | null>(null)

async function handle_submit(payload: { email: string; password: string }): Promise<void> {
  if (loading.value) return
  loading.value = true
  server_error.value = null
  const result = await register(payload.email, payload.password)
  loading.value = false
  if (!result.ok) {
    server_error.value = result.error !== undefined ? result.error : 'Registration failed'
    return
  }
  await router.push({ name: 'dashboard' })
}
</script>

<template>
  <PageShell layout="bleed">
    <AuthShell title="Create an account">
      <RegisterCard
        :loading="loading"
        :server-error="server_error"
        @submit="handle_submit"
      />
      <template #footer>
        Already have an account?
        <router-link :to="{ name: 'auth.login' }" class="register-link">Sign in</router-link>
      </template>
    </AuthShell>
  </PageShell>
</template>

<style scoped>
@layer app {
  .register-link {
    color: var(--p-primary-color);
    font-weight: 500;
  }

  .register-link:hover {
    text-decoration: underline;
  }
}
</style>
