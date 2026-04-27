<script setup lang="ts">
import { useRouter } from 'vue-router'
import PageShell from '@/components/PageShell.vue'
import { useAuth } from '@/composables/auth'

const router = useRouter()
const { is_authenticated } = useAuth()

function go_login(): void {
  router.push({ name: 'auth.login' })
}

function go_register(): void {
  router.push({ name: 'auth.register' })
}

function go_dashboard(): void {
  router.push({ name: 'dashboard' })
}
</script>

<template>
  <PageShell layout="bleed">
    <div class="welcome-root">
      <section class="welcome-hero">
        <div class="welcome-hero-inner">
          <h1 class="welcome-headline">The Rust stack that ships.</h1>
          <p class="welcome-sub">
            Catablast is a strongly-typed Axum + Vue 3 monolith for small-to-mid SaaS. One binary.
            One database. Heavy codegen. No magic.
          </p>
          <div class="welcome-cta-row">
            <template v-if="is_authenticated">
              <Button label="Go to dashboard" @click="go_dashboard" />
            </template>
            <template v-else>
              <Button label="Get started" @click="go_register" />
              <Button label="Sign in" severity="secondary" @click="go_login" />
            </template>
          </div>
        </div>
      </section>

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

  .welcome-hero {
    display: flex;
    align-items: center;
    justify-content: center;
    padding-block: var(--app-pad-section-md);
    padding-inline: var(--app-space-xl);
    background: var(--p-content-background);
    text-align: center;
    flex: 1 1 auto;
  }

  .welcome-hero-inner {
    max-width: var(--app-container-md);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--app-space-3xl);
  }

  .welcome-headline {
    margin: 0;
    font-size: var(--app-fs-display-lg);
    font-weight: 700;
    line-height: 1.1;
    color: var(--p-text-color);
  }

  .welcome-sub {
    margin: 0;
    font-size: var(--app-fs-body-resp);
    color: var(--p-text-muted-color);
    max-width: var(--app-container-sm);
    line-height: 1.6;
  }

  .welcome-cta-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--app-space-md);
    justify-content: center;
  }

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
