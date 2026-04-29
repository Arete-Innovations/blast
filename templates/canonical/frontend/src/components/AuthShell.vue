<script setup lang="ts">
defineProps<{
  title: string
}>()
</script>

<template>
  <div class="auth-shell">
    <div class="auth-shell-card">
      <h1 class="auth-shell-title">{{ title }}</h1>
      <slot />
      <p class="auth-shell-footer">
        <slot name="footer" />
      </p>
    </div>
  </div>
</template>

<style scoped>
@layer app {
  .auth-shell {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: var(--app-space-md);
    background: var(--p-content-background);
  }

  .auth-shell-card {
    width: 100%;
    max-width: var(--app-container-xs);
    background: var(--p-surface-0);
    border: 0.0625rem solid var(--p-content-border-color);
    border-radius: var(--app-radius-lg);
    padding: var(--app-space-xl);
    display: flex;
    flex-direction: column;
    gap: var(--app-space-lg);
  }

  .auth-shell-title {
    margin: 0;
    font-size: var(--app-fs-2xl);
    font-weight: 700;
    color: var(--p-text-color);
    text-align: center;
  }

  .auth-shell-footer {
    margin: 0;
    text-align: center;
    font-size: var(--app-fs-sm);
    color: var(--p-text-muted-color);
  }
}
</style>
