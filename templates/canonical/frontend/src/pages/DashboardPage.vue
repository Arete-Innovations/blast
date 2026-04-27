<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import { useAuth } from '@/composables/auth'
import Card from 'primevue/card'
import Button from 'primevue/button'

const router = useRouter()
const { current_user, logout, refresh } = useAuth()

const is_loading = ref(true)

onMounted(async () => {
  await refresh()
  is_loading.value = false
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

    <div v-if="is_loading" class="dashboard-loading">
      <span class="dashboard-loading-text">Loading…</span>
    </div>

    <div v-else-if="current_user === null" class="dashboard-empty">
      <p class="dashboard-empty-text">User data unavailable.</p>
    </div>

    <template v-else>
      <Card class="dashboard-user-card">
        <template #title>
          <span class="dashboard-card-title">Account</span>
        </template>
        <template #content>
          <dl class="dashboard-user-fields">
            <div class="dashboard-user-field">
              <dt class="dashboard-field-label">Email</dt>
              <dd class="dashboard-field-value">{{ current_user.email }}</dd>
            </div>
            <div class="dashboard-user-field">
              <dt class="dashboard-field-label">Role</dt>
              <dd class="dashboard-field-value dashboard-field-value--role">{{ current_user.role }}</dd>
            </div>
            <div class="dashboard-user-field">
              <dt class="dashboard-field-label">ID</dt>
              <dd class="dashboard-field-value dashboard-field-value--muted">{{ current_user.id }}</dd>
            </div>
          </dl>
        </template>
        <template #footer>
          <router-link :to="{ name: 'profile' }" class="dashboard-profile-link">
            Edit profile
          </router-link>
        </template>
      </Card>
    </template>
  </PageShell>
</template>

<style scoped>
@layer app {
  .dashboard-title {
    margin: 0;
    font-size: var(--app-text-lg);
    font-weight: var(--app-font-weight-semibold);
    color: var(--p-text-color);
  }

  .dashboard-logout-btn {
    margin-inline-start: auto;
  }

  .dashboard-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--app-space-xl);
  }

  .dashboard-loading-text {
    color: var(--p-text-muted-color);
    font-size: var(--app-text-sm);
  }

  .dashboard-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--app-space-xl);
  }

  .dashboard-empty-text {
    margin: 0;
    color: var(--p-text-muted-color);
    font-size: var(--app-text-sm);
  }

  .dashboard-user-card {
    max-width: 32rem;
  }

  .dashboard-card-title {
    font-size: var(--app-text-md);
    font-weight: var(--app-font-weight-semibold);
    color: var(--p-text-color);
  }

  .dashboard-user-fields {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-sm);
    margin: 0;
    padding: 0;
  }

  .dashboard-user-field {
    display: flex;
    align-items: baseline;
    gap: var(--app-space-sm);
  }

  .dashboard-field-label {
    min-width: 4rem;
    font-size: var(--app-text-sm);
    font-weight: var(--app-font-weight-semibold);
    color: var(--p-text-muted-color);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .dashboard-field-value {
    margin: 0;
    font-size: var(--app-text-sm);
    color: var(--p-text-color);
  }

  .dashboard-field-value--role {
    font-size: var(--app-text-xs);
    font-weight: var(--app-font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--p-primary-color);
  }

  .dashboard-field-value--muted {
    color: var(--p-text-muted-color);
    font-size: var(--app-text-xs);
  }

  .dashboard-profile-link {
    display: inline-block;
    font-size: var(--app-text-sm);
    color: var(--p-primary-color);
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }

  .dashboard-profile-link:hover {
    color: var(--p-primary-hover-color);
  }
}
</style>
