<script setup lang="ts">
defineProps<{
  inputId: string
  label: string
  invalid: boolean
  error: string | undefined
}>()
</script>

<template>
  <div class="auth-field">
    <label :for="inputId" class="auth-label">{{ label }}</label>
    <slot />
    <small v-if="invalid && error !== undefined" class="auth-field-error">
      {{ error }}
    </small>
  </div>
</template>

<style scoped>
@layer app {
  .auth-field {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-xs);
  }

  .auth-label {
    font-size: var(--app-fs-sm);
    font-weight: 600;
    color: var(--p-text-color);
  }

  .auth-field-error {
    color: var(--p-red-500, var(--p-danger-color));
    font-size: var(--app-fs-xs);
  }
}
</style>
