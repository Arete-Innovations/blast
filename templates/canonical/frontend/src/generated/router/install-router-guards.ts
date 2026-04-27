import type {
  NavigationGuardNext,
  RouteLocationNormalized,
  Router,
} from 'vue-router';

export type Role = 'user' | 'admin';

export interface InstallRouterGuardsOptions {
  resolveRole?: () => Role | null;
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
