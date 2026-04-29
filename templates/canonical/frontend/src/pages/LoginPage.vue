<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import AuthShell from '@/components/AuthShell.vue'
import LoginCard from '@/components/LoginCard.vue'
import { useAuth } from '@/composables/auth'

const router = useRouter()
const auth = useAuth()

const loading = ref(false)
const server_error = ref<string | null>(null)

async function handle_submit(payload: { email: string; password: string }): Promise<void> {
  if (loading.value) return
  server_error.value = null
  loading.value = true
  try {
    const result = await auth.login(payload.email, payload.password)
    if (result.ok) {
      await router.push({ name: 'dashboard' })
    } else {
      server_error.value = result.error !== undefined ? result.error : 'Login failed'
    }
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <PageShell layout="bleed">
    <AuthShell title="Sign in">
      <LoginCard
        :loading="loading"
        :server-error="server_error"
        @submit="handle_submit"
      />
      <template #footer>
        Don't have an account?
        <router-link :to="{ name: 'auth.register' }" class="login-register-link">Create one</router-link>
      </template>
    </AuthShell>
  </PageShell>
</template>

<style scoped>
@layer app {
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
