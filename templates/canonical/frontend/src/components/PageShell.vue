<script setup lang="ts">
// PageShell — every page wraps content in this. Layout is enum-locked;
// the layout chosen owns the spacing. No padding/margin/gap props are
// accepted — pick a layout, layout owns the spacing (per SPEC_FRONTEND).
//
// Layouts:
//   cards   — default; padded; gap between sections
//   split   — master-detail; rail attaches to sidebar; right-padded only
//   table   — zero padding; full viewport height; child table scrolls
//   bleed   — zero everything; full viewport; component owns chrome
//   tabbed  — tab container; child <RouterView> renders inside

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

  /* cards — default. padding all around, gap between stacked sections */
  .page-shell[data-layout='cards'] .page-shell-body {
    gap: var(--app-space-md);
    padding: var(--app-space-md);
  }

  /* split — master-detail. flex row that stacks on narrow viewport */
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

  /* table — full viewport, zero padding, child owns scroll */
  .page-shell[data-layout='table'] {
    height: 100vh;
    overflow: hidden;
  }

  .page-shell[data-layout='table'] .page-shell-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }

  /* bleed — zero everything, full viewport */
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

  /* tabbed — tab container; child <RouterView> picks its own layout */
  .page-shell[data-layout='tabbed'] .page-shell-body {
    padding-block-start: 0;
    padding-block-end: var(--app-space-md);
    padding-inline: var(--app-space-md);
    gap: var(--app-space-md);
  }
}
</style>
