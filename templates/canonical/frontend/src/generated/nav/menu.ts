import type { RouteName } from '@/generated/router/route-names';

export type Role = 'user' | 'admin';

export interface MenuEntry {
  readonly route: RouteName;
  readonly label: string | null;
  readonly icon: string | null;
  readonly roles: readonly Role[] | null;
}

export interface MenuSection {
  readonly key: string;
  readonly label: string;
  readonly icon: string;
  readonly roles: readonly Role[] | null;
  readonly entries: readonly MenuEntry[];
}

export const NAV: readonly MenuSection[] = [];
