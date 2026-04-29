<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import DashboardUserCard from '@/components/DashboardUserCard.vue'
import { useAuth } from '@/composables/auth'
import Button from 'primevue/button'

const router = useRouter()
const { current_user, logout, refresh } = useAuth()

onMounted(() => {
  refresh()
})

async function handle_logout(): Promise<void> {
  await logout()
  await router.push({ name: 'welcome' })
}
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <h1 class="dashboard-title">Dashboard</h1>
      <Button
        label="Logout"
        severity="secondary"
        class="dashboard-logout-btn"
        @click="handle_logout"
      />
    </template>

    <p v-if="current_user === null" class="dashboard-empty-text">
      User data unavailable.
    </p>

    <DashboardUserCard v-else :user="current_user" />
  </PageShell>
</template>

<style scoped>
@layer app {
  .dashboard-title {
    font-size: var(--app-fs-lg);
    font-weight: 600;
    color: var(--p-text-color);
  }

  .dashboard-logout-btn {
    margin-inline-start: auto;
  }

  .dashboard-empty-text {
    color: var(--p-text-muted-color);
    font-size: var(--app-fs-sm);
  }
}
</style>
