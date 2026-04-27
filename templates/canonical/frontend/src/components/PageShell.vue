<script setup lang="ts">
export type PageLayout = 'cards' | 'split' | 'table' | 'bleed' | 'tabbed'

defineProps<{ layout: PageLayout }>()
</script>

<template>
  <section class="page-shell" :data-layout="layout">
    <header v-if="$slots.header" class="page-shell-header">
      <slot name="header" />
    </header>
    <div class="page-shell-body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
@layer app {
  .page-shell {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    background: var(--p-content-background);
    color: var(--p-text-color);
  }

  .page-shell-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--app-space-md);
    padding-block: var(--app-space-md);
    padding-inline: var(--app-space-md);
    border-bottom: 0.0625rem solid var(--p-content-border-color);
  }

  .page-shell-body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
  }

  .page-shell[data-layout='cards'] .page-shell-body {
    gap: var(--app-space-md);
    padding: var(--app-space-md);
  }

  .page-shell[data-layout='split'] .page-shell-body {
    display: flex;
    flex-direction: row;
    gap: var(--app-space-md);
    padding-inline-end: var(--app-space-md);
    padding-inline-start: 0;
  }

  @media (max-width: 48rem) {
    .page-shell[data-layout='split'] .page-shell-body {
      flex-direction: column;
      padding-inline-end: var(--app-space-md);
      padding-inline-start: var(--app-space-md);
    }
  }

  .page-shell[data-layout='table'] {
    height: 100vh;
    overflow: hidden;
  }

  .page-shell[data-layout='table'] .page-shell-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }

  .page-shell[data-layout='bleed'] {
    height: 100vh;
    overflow: hidden;
  }

  .page-shell[data-layout='bleed'] .page-shell-header {
    display: none;
  }

  .page-shell[data-layout='bleed'] .page-shell-body {
    flex: 1 1 auto;
  }

  .page-shell[data-layout='tabbed'] .page-shell-body {
    padding-block-start: 0;
    padding-block-end: var(--app-space-md);
    padding-inline: var(--app-space-md);
    gap: var(--app-space-md);
  }
}
</style>
