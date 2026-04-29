<script setup lang="ts">
import Card from 'primevue/card'
import Button from 'primevue/button'
import type { AuthUser } from '@/composables/auth'

defineProps<{
  user: AuthUser
}>()

const emit = defineEmits<{ (e: 'logout'): void }>()
</script>

<template>
  <Card class="profile-card">
    <template #content>
      <dl class="profile-fields">
        <div class="profile-field">
          <dt class="profile-field-label">ID</dt>
          <dd class="profile-field-value">{{ user.id }}</dd>
        </div>
        <div class="profile-field">
          <dt class="profile-field-label">Email</dt>
          <dd class="profile-field-value">{{ user.email }}</dd>
        </div>
        <div class="profile-field">
          <dt class="profile-field-label">Role</dt>
          <dd class="profile-field-value">{{ user.role }}</dd>
        </div>
      </dl>
    </template>
    <template #footer>
      <Button
        label="Log out"
        severity="danger"
        class="profile-logout-btn"
        @click="emit('logout')"
      />
    </template>
  </Card>
</template>

<style scoped>
@layer app {
  .profile-card {
    max-width: var(--app-container-sm);
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
    font-weight: 500;
    color: var(--p-text-muted-color);
    font-size: var(--app-fs-sm);
    align-self: center;
  }

  .profile-field-value {
    color: var(--p-text-color);
    font-size: var(--app-fs-sm);
    margin: 0;
    word-break: break-all;
    align-self: center;
  }

  .profile-logout-btn {
    margin-block-start: var(--app-space-sm);
  }
}
</style>
