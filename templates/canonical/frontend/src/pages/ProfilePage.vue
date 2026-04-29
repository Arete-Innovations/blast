<script setup lang="ts">
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import ProfileCard from '@/components/ProfileCard.vue'
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

    <p v-if="current_user === null" class="profile-loading-text">
      Loading account details&hellip; If this persists, please refresh.
    </p>

    <ProfileCard v-else :user="current_user" @logout="handle_logout" />
  </PageShell>
</template>

<style scoped>
@layer app {
  .profile-back-link {
    font-size: var(--app-fs-sm);
    color: var(--p-text-muted-color);
  }

  .profile-back-link:hover {
    color: var(--p-text-color);
  }

  .profile-loading-text {
    color: var(--p-text-muted-color);
    font-size: var(--app-fs-sm);
  }
}
</style>
