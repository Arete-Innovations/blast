<script setup lang="ts">
import { useRouter } from 'vue-router'
import Button from 'primevue/button'
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
</template>

<style scoped>
@layer app {
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
}
</style>
