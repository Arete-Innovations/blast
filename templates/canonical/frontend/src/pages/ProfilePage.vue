<script setup lang="ts">
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import { useAuth } from '@/composables/auth'

const { current_user, logout } = useAuth()
const router = useRouter()

async function handle_logout(): Promise<void> {
  await logout()
  await router.push({ name: 'welcome' })
}
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <h1>Profile</h1>
      <router-link :to="{ name: 'dashboard' }" class="profile-back-link">
        &larr; Dashboard
      </router-link>
    </template>

    <div v-if="current_user === null" class="profile-loading">
      <p class="profile-loading-text">Loading account details&hellip; If this persists, please refresh.</p>
    </div>

    <Card v-else class="profile-card">
      <template #content>
        <dl class="profile-fields">
          <div class="profile-field">
            <dt class="profile-field-label">ID</dt>
            <dd class="profile-field-value">{{ current_user.id }}</dd>
          </div>
          <div class="profile-field">
            <dt class="profile-field-label">Email</dt>
            <dd class="profile-field-value">{{ current_user.email }}</dd>
          </div>
          <div class="profile-field">
            <dt class="profile-field-label">Role</dt>
            <dd class="profile-field-value">{{ current_user.role }}</dd>
          </div>
        </dl>
      </template>
      <template #footer>
        <Button
          label="Log out"
          severity="danger"
          class="profile-logout-btn"
          @click="handle_logout"
        />
      </template>
    </Card>
  </PageShell>
</template>

<style scoped>
@layer app {
  .profile-back-link {
    font-size: var(--app-font-size-sm);
    color: var(--p-text-muted-color);
  }

  .profile-back-link:hover {
    color: var(--p-text-color);
  }

  .profile-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--app-space-lg);
  }

  .profile-loading-text {
    color: var(--p-text-muted-color);
    font-size: var(--app-font-size-sm);
  }

  .profile-card {
    max-width: 32rem;
  }

  .profile-fields {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--app-space-xs) var(--app-space-md);
    margin: 0;
    padding: 0;
  }

  .profile-field {
    display: contents;
  }

  .profile-field-label {
    font-weight: var(--app-font-weight-medium);
    color: var(--p-text-muted-color);
    font-size: var(--app-font-size-sm);
    align-self: center;
  }

  .profile-field-value {
    color: var(--p-text-color);
    font-size: var(--app-font-size-sm);
    margin: 0;
    word-break: break-all;
    align-self: center;
  }

  .profile-logout-btn {
    margin-block-start: var(--app-space-sm);
  }
}
</style>
