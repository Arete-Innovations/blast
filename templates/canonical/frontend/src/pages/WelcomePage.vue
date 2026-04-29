<script setup lang="ts">
import PageShell from '@/components/PageShell.vue'
import WelcomeHero from '@/components/WelcomeHero.vue'
import WelcomeFeatures from '@/components/WelcomeFeatures.vue'
</script>

<template>
  <PageShell layout="bleed">
    <div class="welcome-root">
      <WelcomeHero />
      <WelcomeFeatures />
    </div>
  </PageShell>
</template>

<style scoped>
@layer app {
  .welcome-root {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
  }
}
</style>
