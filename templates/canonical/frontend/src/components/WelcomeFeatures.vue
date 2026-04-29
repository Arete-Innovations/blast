<script setup lang="ts">
import Card from 'primevue/card'
</script>

<template>
  <section class="welcome-features">
    <div class="welcome-features-inner">
      <Card class="welcome-feature-card">
        <template #title>Type-safe by default</template>
        <template #content>
          <p>Schema drives codegen for both Rust structs and TypeScript types. If the contract
          drifts, the build fails — not production.</p>
        </template>
      </Card>

      <Card class="welcome-feature-card">
        <template #title>One binary, one VPS</template>
        <template #content>
          <p>The backend is a single Axum binary. The Vue bundle is packed in. Deploy with a
          systemd unit. No containers required.</p>
        </template>
      </Card>

      <Card class="welcome-feature-card">
        <template #title>Fork, don't configure</template>
        <template #content>
          <p>Blast vendors the entire framework into your project. There is no plugin system.
          Change anything you like — it's your code now.</p>
        </template>
      </Card>
    </div>
  </section>
</template>

<style scoped>
@layer app {
  .welcome-features {
    padding-block: var(--app-pad-section-sm);
    padding-inline: var(--app-space-xl);
    background: var(--p-surface-50);
  }

  .welcome-features-inner {
    max-width: var(--app-container-2xl);
    margin-inline: auto;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(var(--app-container-xs), 1fr));
    gap: var(--app-space-4xl);
  }

  .welcome-feature-card {
    border-radius: var(--app-radius-lg);
  }
}
</style>
