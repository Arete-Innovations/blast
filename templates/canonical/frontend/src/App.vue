<script setup lang="ts">
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
}
</style>
