<script setup lang="ts">
import { useRoute, RouterLink } from 'vue-router'
import Button from 'primevue/button'

const route = useRoute()
</script>

<template>
  <div class="not-found">
    <span class="not-found__code" aria-hidden="true">404</span>
    <h1 class="not-found__heading">Page not found</h1>
    <p class="not-found__body">
      The page at
      <code class="not-found__path">{{ route.fullPath }}</code>
      doesn't exist or has been moved.
    </p>
    <RouterLink :to="{ name: 'welcome' }" custom v-slot="{ navigate }">
      <Button label="Back to home" @click="navigate" />
    </RouterLink>
  </div>
</template>

<style scoped>
@layer app {
  .not-found {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--app-space-4xl);
    min-height: 100vh;
    padding: var(--app-space-5xl) var(--app-space-xl);
    text-align: center;
  }

  .not-found__code {
    font-size: var(--app-fs-display-lg);
    font-weight: 800;
    line-height: 1;
    color: var(--p-primary-color);
    letter-spacing: -0.04em;
  }

  .not-found__heading {
    margin: 0;
    font-size: var(--app-fs-h1-resp);
    font-weight: 600;
    color: var(--p-text-color);
  }

  .not-found__body {
    margin: 0;
    font-size: var(--app-fs-body-resp);
    color: var(--p-text-muted-color);
    max-width: var(--app-container-sm);
  }

  .not-found__path {
    font-family: var(--app-font-mono);
    font-size: var(--app-fs-sm);
    background: var(--p-content-border-color);
    padding-block: var(--app-space-2xs);
    padding-inline: var(--app-space-xs);
    border-radius: var(--app-radius-sm);
  }
}
</style>
