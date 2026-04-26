// Auto-generated. Do not edit by hand.
// Auth + role gating consumed from route.meta.roles.
//
// Wire this into your router from `main.ts` (or `router/index.ts` per the
// fe_runtime scaffold). Pass an optional `resolveRole` callback that
// returns the current user's role from your session store; without it,
// any role-guarded route blocks until you wire a session adapter.
//
// Usage:
//
//   import router from '@/router';
//   import { installRouterGuards } from '@/generated/router/install-router-guards';
//   import { useSession } from '@/composables/session';
//
//   const session = useSession();
//   installRouterGuards(router, {
//     resolveRole: () => session.role.value,
//     redirectName: 'dashboard',
//   });
//
// When invoked without options (`installRouterGuards(router)`), public
// routes pass freely; role-guarded routes always cancel navigation
// because there is no role resolver to consult.

import type {
  NavigationGuardNext,
  RouteLocationNormalized,
  Router,
} from 'vue-router';

export type Role = 'user' | 'admin';

export interface InstallRouterGuardsOptions {
  /// Returns the current user's role, or `null` when unauthenticated.
  /// The guard never inspects session state directly — keeps this file
  /// dependency-free.
  resolveRole?: () => Role | null;
  /// Route name to redirect to when an unauthenticated user hits a
  /// guarded route. Default: do not redirect; cancel navigation.
  redirectName?: string;
}

export function installRouterGuards(
  router: Router,
  opts?: InstallRouterGuardsOptions,
): void {
  const options: InstallRouterGuardsOptions = opts !== undefined ? opts : {};
  router.beforeEach((to: RouteLocationNormalized, _from: RouteLocationNormalized, next: NavigationGuardNext) => {
    const meta = to.meta as { roles?: readonly Role[] | null };
    const required = meta.roles;

    if (required === undefined || required === null) {
      next();
      return;
    }

    const resolver = options.resolveRole;
    if (resolver === undefined) {
      if (options.redirectName !== undefined) {
        next({ name: options.redirectName });
        return;
      }
      next(false);
      return;
    }

    const current = resolver();
    if (current === null) {
      if (options.redirectName !== undefined) {
        next({ name: options.redirectName });
        return;
      }
      next(false);
      return;
    }

    if (required.length === 0) {
      next();
      return;
    }

    if (required.includes(current)) {
      next();
      return;
    }

    if (options.redirectName !== undefined) {
      next({ name: options.redirectName });
      return;
    }
    next(false);
  });
}
