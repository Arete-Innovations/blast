<script setup lang="ts">
// App — root component. Renders the global progress bar and the active
// route. Layout chrome (sidebar/topbar/breadcrumb) lives in
// `@/custom/components/AppChrome.vue` so users can extend without
// touching this framework file. AppChrome provides a default-slot that
// wraps the RouterView.

import GlobalProgressBar from '@/components/GlobalProgressBar.vue'
import AppChrome from '@/custom/components/AppChrome.vue'
</script>

<template>
  <GlobalProgressBar />
  <AppChrome>
    <RouterView />
  </AppChrome>
</template>

<style scoped>
@layer app {
  /* Root component holds no layout styles — chrome is up to AppChrome,
     spacing is up to PageShell. */
}
</style>
