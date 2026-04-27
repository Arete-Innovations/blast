import type { RouteRecordRaw } from 'vue-router';

export interface RouteMeta {
  readonly layout: 'cards' | 'split' | 'table' | 'bleed' | 'tabbed';
  readonly label: string | null;
  readonly icon: string | null;
  readonly roles: readonly ('user' | 'admin')[] | null;
}

export const routes: readonly RouteRecordRaw[] = [];
