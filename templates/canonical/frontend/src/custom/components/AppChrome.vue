<script setup lang="ts">
// AppChrome — app-level chrome (sidebar, topbar, breadcrumb, footer).
// Default is a transparent passthrough — the slot is the active route's
// rendered view. Replace this template to add chrome without touching
// `App.vue` (a framework file).
</script>

<template>
  <slot />
</template>

<style scoped>
@layer app {
  /* Add your chrome styles here. */
}
</style>
