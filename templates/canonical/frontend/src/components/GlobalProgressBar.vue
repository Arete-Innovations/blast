<script setup lang="ts">
import { useGlobalProgress } from '@/composables/global-progress'

const { isActive } = useGlobalProgress()
</script>

<template>
  <div
    class="global-progress-bar"
    role="progressbar"
    aria-live="polite"
    aria-label="Loading"
    :data-active="isActive"
  >
    <div class="global-progress-bar-fill" />
  </div>
</template>

<style scoped>
@layer app {
  .global-progress-bar {
    position: fixed;
    top: 0;
    inset-inline: 0;
    height: 0.1875rem;
    overflow: hidden;
    z-index: var(--app-z-overlay);
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--app-transition-fast);
  }

  .global-progress-bar[data-active='true'] {
    opacity: 1;
  }

  .global-progress-bar-fill {
    width: 40%;
    height: 100%;
    background: var(--p-primary-color);
    transform: translateX(-100%);
    will-change: transform;
  }

  .global-progress-bar[data-active='true'] .global-progress-bar-fill {
    animation: global-progress-bar-slide 1.1s linear infinite;
  }

  @keyframes global-progress-bar-slide {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(350%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .global-progress-bar-fill {
      animation: none;
    }
  }
}
</style>
