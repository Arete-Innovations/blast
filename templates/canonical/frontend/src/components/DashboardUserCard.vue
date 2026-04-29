<script setup lang="ts">
import Card from 'primevue/card'
import type { AuthUser } from '@/composables/auth'

defineProps<{ user: AuthUser }>()
</script>

<template>
  <Card class="dashboard-user-card">
    <template #title>
      <span class="dashboard-card-title">Account</span>
    </template>
    <template #content>
      <dl class="dashboard-user-fields">
        <div class="dashboard-user-field">
          <dt class="dashboard-field-label">Email</dt>
          <dd class="dashboard-field-value">{{ user.email }}</dd>
        </div>
        <div class="dashboard-user-field">
          <dt class="dashboard-field-label">Role</dt>
          <dd class="dashboard-field-value dashboard-field-value--role">{{ user.role }}</dd>
        </div>
        <div class="dashboard-user-field">
          <dt class="dashboard-field-label">ID</dt>
          <dd class="dashboard-field-value dashboard-field-value--muted">{{ user.id }}</dd>
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

<style scoped>
@layer app {
  .dashboard-user-card {
    max-width: var(--app-container-sm);
  }

  .dashboard-card-title {
    font-size: var(--app-fs-md);
    font-weight: 600;
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
    min-width: var(--app-icon-2xl);
    font-size: var(--app-fs-sm);
    font-weight: 600;
    color: var(--p-text-muted-color);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .dashboard-field-value {
    margin: 0;
    font-size: var(--app-fs-sm);
    color: var(--p-text-color);
  }

  .dashboard-field-value--role {
    font-size: var(--app-fs-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--p-primary-color);
  }

  .dashboard-field-value--muted {
    color: var(--p-text-muted-color);
    font-size: var(--app-fs-xs);
  }

  .dashboard-profile-link {
    display: inline-block;
    font-size: var(--app-fs-sm);
    color: var(--p-primary-color);
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }

  .dashboard-profile-link:hover {
    color: var(--p-primary-hover-color);
  }
}
</style>
