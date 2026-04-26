//! Static FE composable bodies (URL state, dialog/drawer URL binding,
//! Relay channel subscription). Split from `fe_runtime.rs` to keep file
//! sizes under the build.rs decomposition cap. These are scaffold seeds
//! — `write_if_absent`, identical contract to the rest of `fe_runtime`.
//!
//! Per SPEC_FRONTEND_ROUTING.md:
//!   - Modals/drawers are URL state, not local refs.
//!   - List view state (page/page_size/sort/filter) is URL state.
//!
//! Per SPEC_RELAY.md:
//!   - WS subscriptions go through the singleton WsClient, never raw
//!     `new WebSocket(...)`. `useChannel` is the low-level primitive
//!     per-resource composables build on for Tier-3 (live) refetches.

pub const URL_TS: &str = URL_TS_BODY;
pub const DIALOG_TS: &str = DIALOG_TS_BODY;
pub const DRAWER_TS: &str = DRAWER_TS_BODY;
pub const CHANNEL_TS: &str = CHANNEL_TS_BODY;

const URL_TS_BODY: &str = r#"// url — URL-state binding helpers. SPAs broke the web's URL contract;
// these put view state back where it belongs (the URL) so refresh,
// share-link, back/forward all behave like a respectable MPA. See
// SPEC_FRONTEND_ROUTING.md (`useUrlListState`) and SPEC_FRONTEND.md
// (list wire schema) for the contract this enforces.
//
// Two exports:
//   useQueryParam<T>(name, opts?) — bind a single query param to a ref.
//   useUrlListState(opts?)        — page/page_size/sort/filter bound to
//                                   the canonical list-endpoint contract.
//
// History mode is configurable per-call: `push` adds a history entry
// (default — back button reverses the change); `replace` swaps in
// place (use for transient/auto-driven updates that shouldn't pollute
// history).

import { computed, watch } from 'vue'
import type { ComputedRef, WritableComputedRef } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { LocationQueryRaw } from 'vue-router'

export type QueryHistoryMode = 'push' | 'replace'

export interface UseQueryParamOptions<T> {
  default?: T
  history?: QueryHistoryMode
}

export interface UrlListState {
  page: WritableComputedRef<number>
  pageSize: WritableComputedRef<number>
  sort: WritableComputedRef<string>
  filter: WritableComputedRef<Record<string, string>>
}

export interface UseUrlListStateOptions {
  defaultPageSize?: number
  defaultSort?: string
  history?: QueryHistoryMode
}

const DEFAULT_PAGE = 1
const DEFAULT_PAGE_SIZE = 25
const MIN_PAGE_SIZE = 1
const MAX_PAGE_SIZE = 200
const DEFAULT_SORT = '+id'
const FILTER_PREFIX = 'filter['
const FILTER_SUFFIX = ']'

export function useQueryParam<T extends string | number | boolean>(
  name: string,
  opts?: UseQueryParamOptions<T>,
): WritableComputedRef<T | null> {
  const route = useRoute()
  const router = useRouter()

  const default_value: T | null = opts !== undefined && opts.default !== undefined ? opts.default : null
  const history_mode: QueryHistoryMode = opts !== undefined && opts.history !== undefined ? opts.history : 'push'

  return computed<T | null>({
    get(): T | null {
      const raw = route.query[name]
      if (raw === undefined || raw === null) {
        return default_value
      }
      const str = Array.isArray(raw) ? raw[0] : raw
      if (str === null || str === undefined) {
        return default_value
      }
      return parse_value<T>(str, default_value)
    },
    set(value: T | null): void {
      const next: LocationQueryRaw = { ...route.query }
      if (value === null || value === undefined) {
        delete next[name]
      } else {
        next[name] = encode_value(value)
      }
      void apply_query(router, next, history_mode)
    },
  })
}

export function useUrlListState(opts?: UseUrlListStateOptions): UrlListState {
  const route = useRoute()
  const router = useRouter()

  const default_page_size: number = opts !== undefined && opts.defaultPageSize !== undefined
    ? clamp_page_size(opts.defaultPageSize)
    : DEFAULT_PAGE_SIZE
  const default_sort: string = opts !== undefined && opts.defaultSort !== undefined
    ? opts.defaultSort
    : DEFAULT_SORT
  const history_mode: QueryHistoryMode = opts !== undefined && opts.history !== undefined
    ? opts.history
    : 'push'

  const page = computed<number>({
    get(): number {
      const raw = single_query_value(route.query.page)
      if (raw === null) {
        return DEFAULT_PAGE
      }
      const parsed = Number.parseInt(raw, 10)
      if (Number.isNaN(parsed) || parsed < 1) {
        return DEFAULT_PAGE
      }
      return parsed
    },
    set(value: number): void {
      const next: LocationQueryRaw = { ...route.query }
      if (value === DEFAULT_PAGE) {
        delete next.page
      } else {
        next.page = String(value)
      }
      void apply_query(router, next, history_mode)
    },
  })

  const pageSize = computed<number>({
    get(): number {
      const raw = single_query_value(route.query.page_size)
      if (raw === null) {
        return default_page_size
      }
      const parsed = Number.parseInt(raw, 10)
      if (Number.isNaN(parsed)) {
        return default_page_size
      }
      return clamp_page_size(parsed)
    },
    set(value: number): void {
      const clamped = clamp_page_size(value)
      const next: LocationQueryRaw = { ...route.query }
      if (clamped === default_page_size) {
        delete next.page_size
      } else {
        next.page_size = String(clamped)
      }
      void apply_query(router, next, history_mode)
    },
  })

  const sort = computed<string>({
    get(): string {
      const raw = single_query_value(route.query.sort)
      if (raw === null) {
        return default_sort
      }
      return raw
    },
    set(value: string): void {
      const next: LocationQueryRaw = { ...route.query }
      if (value === default_sort) {
        delete next.sort
      } else {
        next.sort = value
      }
      void apply_query(router, next, history_mode)
    },
  })

  const filter = computed<Record<string, string>>({
    get(): Record<string, string> {
      const out: Record<string, string> = {}
      for (const key of Object.keys(route.query)) {
        if (!key.startsWith(FILTER_PREFIX) || !key.endsWith(FILTER_SUFFIX)) {
          continue
        }
        const col = key.slice(FILTER_PREFIX.length, key.length - FILTER_SUFFIX.length)
        if (col.length === 0) {
          continue
        }
        const raw = single_query_value(route.query[key])
        if (raw === null) {
          continue
        }
        out[col] = raw
      }
      return out
    },
    set(value: Record<string, string>): void {
      const next: LocationQueryRaw = { ...route.query }
      for (const key of Object.keys(next)) {
        if (key.startsWith(FILTER_PREFIX) && key.endsWith(FILTER_SUFFIX)) {
          delete next[key]
        }
      }
      for (const col of Object.keys(value)) {
        const v = value[col]
        if (v === undefined || v === null || v === '') {
          continue
        }
        next[`${FILTER_PREFIX}${col}${FILTER_SUFFIX}`] = v
      }
      void apply_query(router, next, history_mode)
    },
  })

  return { page, pageSize, sort, filter }
}

function parse_value<T extends string | number | boolean>(raw: string, default_value: T | null): T | null {
  if (default_value === null) {
    return raw as T
  }
  const default_type = typeof default_value
  if (default_type === 'number') {
    const parsed = Number(raw)
    if (Number.isNaN(parsed)) {
      return default_value
    }
    return parsed as T
  }
  if (default_type === 'boolean') {
    if (raw === 'true' || raw === '1') {
      return true as T
    }
    if (raw === 'false' || raw === '0') {
      return false as T
    }
    return default_value
  }
  return raw as T
}

function encode_value<T extends string | number | boolean>(value: T): string {
  const t = typeof value
  if (t === 'boolean') {
    return value === true ? 'true' : 'false'
  }
  return String(value)
}

function single_query_value(raw: unknown): string | null {
  if (raw === undefined || raw === null) {
    return null
  }
  if (Array.isArray(raw)) {
    const first = raw[0]
    if (first === undefined || first === null) {
      return null
    }
    return String(first)
  }
  return String(raw)
}

function clamp_page_size(value: number): number {
  if (Number.isNaN(value)) {
    return DEFAULT_PAGE_SIZE
  }
  if (value < MIN_PAGE_SIZE) {
    return MIN_PAGE_SIZE
  }
  if (value > MAX_PAGE_SIZE) {
    return MAX_PAGE_SIZE
  }
  return Math.floor(value)
}

async function apply_query(
  router: ReturnType<typeof useRouter>,
  query: LocationQueryRaw,
  mode: QueryHistoryMode,
): Promise<void> {
  if (mode === 'replace') {
    await router.replace({ query })
    return
  }
  await router.push({ query })
}

// Re-export `watch` so consumers needing to react to URL state changes
// don't have to re-import from 'vue' alongside this module. Keeps the
// composable surface a single import target.
export { watch }
"#;

const DIALOG_TS_BODY: &str = r#"// dialog — modal-as-URL-state composable. Per SPEC_FRONTEND_ROUTING.md
// (Modals are URL state), every dialog reflects in `?dialog=<name>` plus
// optional `?dialog_id=<id>`. Refresh-survival, share-link, back-button-
// closes-modal — all free.
//
// Default `history: 'push'` — opening/closing the dialog adds a history
// entry. Override with `history: 'replace'` for transient overlays where
// the back-button-closes-modal behaviour is undesirable.
//
// Mirrors `useQueryDrawer` (separate `?drawer=` key) so a dialog and a
// drawer can be open simultaneously without colliding.

import { computed } from 'vue'
import type { ComputedRef } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { LocationQueryRaw } from 'vue-router'

export type DialogId = number | string

export type DialogHistoryMode = 'push' | 'replace'

export interface UseQueryDialogParams {
  id?: DialogId
}

export interface UseQueryDialogOptions {
  history?: DialogHistoryMode
}

export interface QueryDialogHandle {
  visible: ComputedRef<boolean>
  id: ComputedRef<DialogId | null>
  open: (params?: UseQueryDialogParams) => void
  close: () => void
}

const DIALOG_KEY = 'dialog'
const DIALOG_ID_KEY = 'dialog_id'

export function useQueryDialog(
  name: string,
  params?: UseQueryDialogParams,
  opts?: UseQueryDialogOptions,
): QueryDialogHandle {
  const route = useRoute()
  const router = useRouter()

  const history_mode: DialogHistoryMode = opts !== undefined && opts.history !== undefined ? opts.history : 'push'
  const initial_id: DialogId | null = params !== undefined && params.id !== undefined ? params.id : null

  const visible = computed<boolean>(() => {
    const raw = route.query[DIALOG_KEY]
    if (raw === undefined || raw === null) {
      return false
    }
    const str = Array.isArray(raw) ? raw[0] : raw
    return str === name
  })

  const id = computed<DialogId | null>(() => {
    if (!visible.value) {
      return null
    }
    const raw = route.query[DIALOG_ID_KEY]
    if (raw === undefined || raw === null) {
      return null
    }
    const str = Array.isArray(raw) ? raw[0] : raw
    if (str === null || str === undefined || str === '') {
      return null
    }
    const as_num = Number(str)
    if (!Number.isNaN(as_num) && /^-?\d+(\.\d+)?$/.test(str)) {
      return as_num
    }
    return str
  })

  function open(next_params?: UseQueryDialogParams): void {
    const next: LocationQueryRaw = { ...route.query }
    next[DIALOG_KEY] = name
    const id_value: DialogId | null = next_params !== undefined && next_params.id !== undefined
      ? next_params.id
      : initial_id
    if (id_value === null) {
      delete next[DIALOG_ID_KEY]
    } else {
      next[DIALOG_ID_KEY] = String(id_value)
    }
    void apply_query(router, next, history_mode)
  }

  function close(): void {
    const next: LocationQueryRaw = { ...route.query }
    if (next[DIALOG_KEY] !== name) {
      // Another dialog is open; do not stomp it.
      return
    }
    delete next[DIALOG_KEY]
    delete next[DIALOG_ID_KEY]
    void apply_query(router, next, history_mode)
  }

  return { visible, id, open, close }
}

async function apply_query(
  router: ReturnType<typeof useRouter>,
  query: LocationQueryRaw,
  mode: DialogHistoryMode,
): Promise<void> {
  if (mode === 'replace') {
    await router.replace({ query })
    return
  }
  await router.push({ query })
}
"#;

const DRAWER_TS_BODY: &str = r#"// drawer — sibling of useQueryDialog for slide-out panels. Same URL-as-
// state contract; uses `?drawer=<name>` and `?drawer_id=<id>` so a
// dialog and a drawer can be open simultaneously without colliding.
// See SPEC_FRONTEND_ROUTING.md.

import { computed } from 'vue'
import type { ComputedRef } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { LocationQueryRaw } from 'vue-router'

export type DrawerId = number | string

export type DrawerHistoryMode = 'push' | 'replace'

export interface UseQueryDrawerParams {
  id?: DrawerId
}

export interface UseQueryDrawerOptions {
  history?: DrawerHistoryMode
}

export interface QueryDrawerHandle {
  visible: ComputedRef<boolean>
  id: ComputedRef<DrawerId | null>
  open: (params?: UseQueryDrawerParams) => void
  close: () => void
}

const DRAWER_KEY = 'drawer'
const DRAWER_ID_KEY = 'drawer_id'

export function useQueryDrawer(
  name: string,
  params?: UseQueryDrawerParams,
  opts?: UseQueryDrawerOptions,
): QueryDrawerHandle {
  const route = useRoute()
  const router = useRouter()

  const history_mode: DrawerHistoryMode = opts !== undefined && opts.history !== undefined ? opts.history : 'push'
  const initial_id: DrawerId | null = params !== undefined && params.id !== undefined ? params.id : null

  const visible = computed<boolean>(() => {
    const raw = route.query[DRAWER_KEY]
    if (raw === undefined || raw === null) {
      return false
    }
    const str = Array.isArray(raw) ? raw[0] : raw
    return str === name
  })

  const id = computed<DrawerId | null>(() => {
    if (!visible.value) {
      return null
    }
    const raw = route.query[DRAWER_ID_KEY]
    if (raw === undefined || raw === null) {
      return null
    }
    const str = Array.isArray(raw) ? raw[0] : raw
    if (str === null || str === undefined || str === '') {
      return null
    }
    const as_num = Number(str)
    if (!Number.isNaN(as_num) && /^-?\d+(\.\d+)?$/.test(str)) {
      return as_num
    }
    return str
  })

  function open(next_params?: UseQueryDrawerParams): void {
    const next: LocationQueryRaw = { ...route.query }
    next[DRAWER_KEY] = name
    const id_value: DrawerId | null = next_params !== undefined && next_params.id !== undefined
      ? next_params.id
      : initial_id
    if (id_value === null) {
      delete next[DRAWER_ID_KEY]
    } else {
      next[DRAWER_ID_KEY] = String(id_value)
    }
    void apply_query(router, next, history_mode)
  }

  function close(): void {
    const next: LocationQueryRaw = { ...route.query }
    if (next[DRAWER_KEY] !== name) {
      return
    }
    delete next[DRAWER_KEY]
    delete next[DRAWER_ID_KEY]
    void apply_query(router, next, history_mode)
  }

  return { visible, id, open, close }
}

async function apply_query(
  router: ReturnType<typeof useRouter>,
  query: LocationQueryRaw,
  mode: DrawerHistoryMode,
): Promise<void> {
  if (mode === 'replace') {
    await router.replace({ query })
    return
  }
  await router.push({ query })
}
"#;

const CHANNEL_TS_BODY: &str = r#"// channel — low-level Relay WS subscription primitive. Per-resource
// composables (Tier-3 live mode) layer on top of this; this composable
// just registers the topic with the singleton WsClient on mount and
// unregisters on unmount. WsClient handles reconnect; it re-subscribes
// to all known topics, then per SPEC_RELAY.md the consumer composable
// re-fetches from the DB on the resulting ack — there is no replay
// buffer.
//
// Returns:
//   lastEvent     — most recent event payload (null until first event).
//   isSubscribed  — true while the topic is registered with WsClient.
//
// `onMessage` callback fires per event AND `lastEvent` updates
// reactively. Either consumption pattern is valid.

import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { ComputedRef, Ref } from 'vue'

import { wsClient } from '@/generated/ws/client'

export interface UseChannelOptions<T> {
  onMessage?: (payload: T) => void
}

export interface ChannelHandle<T> {
  lastEvent: Ref<T | null>
  isSubscribed: ComputedRef<boolean>
}

export function useChannel<T>(topic: string, opts?: UseChannelOptions<T>): ChannelHandle<T> {
  const last_event = ref<T | null>(null) as Ref<T | null>
  const subscribed = ref<boolean>(false)
  const is_subscribed = computed<boolean>(() => subscribed.value)

  function handle(payload: T): void {
    last_event.value = payload
    if (opts !== undefined && opts.onMessage !== undefined) {
      opts.onMessage(payload)
    }
  }

  onMounted(() => {
    wsClient.subscribe<T>(topic, handle)
    subscribed.value = true
  })

  onUnmounted(() => {
    wsClient.unsubscribe(topic, handle)
    subscribed.value = false
  })

  return { lastEvent: last_event, isSubscribed: is_subscribed }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_ts_exports_query_param_and_list_state() {
        assert!(URL_TS.contains("export function useQueryParam"));
        assert!(URL_TS.contains("export function useUrlListState"));
        assert!(URL_TS.contains("page_size"));
        assert!(URL_TS.contains("filter["));
        assert!(URL_TS.contains("MAX_PAGE_SIZE"));
        assert!(URL_TS.contains("DEFAULT_PAGE_SIZE"));
        assert!(URL_TS.contains("DEFAULT_SORT"));
        assert!(URL_TS.contains("router.push"));
        assert!(URL_TS.contains("router.replace"));
        assert!(URL_TS.contains("useRoute"));
        assert!(URL_TS.contains("useRouter"));
        assert!(!URL_TS.contains(": any"));
        assert!(!URL_TS.contains("as any"));
        assert!(!URL_TS.contains("@ts-ignore"));
        assert!(!URL_TS.contains("console.log"));
        assert!(!URL_TS.contains("console.warn"));
        assert!(!URL_TS.contains("console.error"));
    }

    #[test]
    fn dialog_ts_exports_query_dialog() {
        assert!(DIALOG_TS.contains("export function useQueryDialog"));
        assert!(DIALOG_TS.contains("DIALOG_KEY"));
        assert!(DIALOG_TS.contains("DIALOG_ID_KEY"));
        assert!(DIALOG_TS.contains("'dialog'"));
        assert!(DIALOG_TS.contains("'dialog_id'"));
        assert!(DIALOG_TS.contains("visible"));
        assert!(DIALOG_TS.contains("open"));
        assert!(DIALOG_TS.contains("close"));
        assert!(DIALOG_TS.contains("history_mode"));
        assert!(DIALOG_TS.contains("'push'"));
        assert!(DIALOG_TS.contains("'replace'"));
        assert!(!DIALOG_TS.contains(": any"));
        assert!(!DIALOG_TS.contains("as any"));
        assert!(!DIALOG_TS.contains("@ts-ignore"));
        assert!(!DIALOG_TS.contains("console.log"));
    }

    #[test]
    fn drawer_ts_exports_query_drawer_with_independent_keys() {
        assert!(DRAWER_TS.contains("export function useQueryDrawer"));
        assert!(DRAWER_TS.contains("'drawer'"));
        assert!(DRAWER_TS.contains("'drawer_id'"));
        // Drawer keys must not collide with dialog keys.
        assert!(!DRAWER_TS.contains("'dialog'"));
        assert!(!DRAWER_TS.contains("'dialog_id'"));
        assert!(DRAWER_TS.contains("visible"));
        assert!(DRAWER_TS.contains("open"));
        assert!(DRAWER_TS.contains("close"));
        assert!(!DRAWER_TS.contains(": any"));
        assert!(!DRAWER_TS.contains("as any"));
        assert!(!DRAWER_TS.contains("@ts-ignore"));
        assert!(!DRAWER_TS.contains("console.log"));
    }

    #[test]
    fn channel_ts_exports_use_channel() {
        assert!(CHANNEL_TS.contains("export function useChannel"));
        assert!(CHANNEL_TS.contains("@/generated/ws/client"));
        assert!(CHANNEL_TS.contains("wsClient.subscribe"));
        assert!(CHANNEL_TS.contains("wsClient.unsubscribe"));
        assert!(CHANNEL_TS.contains("onMounted"));
        assert!(CHANNEL_TS.contains("onUnmounted"));
        assert!(CHANNEL_TS.contains("lastEvent"));
        assert!(CHANNEL_TS.contains("isSubscribed"));
        assert!(CHANNEL_TS.contains("onMessage"));
        assert!(!CHANNEL_TS.contains(": any"));
        assert!(!CHANNEL_TS.contains("as any"));
        assert!(!CHANNEL_TS.contains("@ts-ignore"));
        assert!(!CHANNEL_TS.contains("console.log"));
        // No raw WebSocket construction; goes through generated client.
        assert!(!CHANNEL_TS.contains("new WebSocket"));
    }

    #[test]
    fn composables_have_no_silent_fallbacks() {
        // SilentFallback rule bans `|| 'literal'`, `?? []`, `?? {}`,
        // `?? 0`, `?? false`. Our composables use explicit if/ternary
        // branching against `undefined`/`null`. Verify none of the
        // banned literal-default patterns leak in.
        for body in [URL_TS, DIALOG_TS, DRAWER_TS, CHANNEL_TS].iter() {
            for line in body.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with("*") {
                    continue;
                }
                assert!(!line.contains("?? []"), "{}", line);
                assert!(!line.contains("?? {}"), "{}", line);
                assert!(!line.contains("?? 0"), "{}", line);
                assert!(!line.contains("?? false"), "{}", line);
                assert!(!line.contains("?? true"), "{}", line);
                assert!(!line.contains("?? ''"), "{}", line);
                assert!(!line.contains("?? \"\""), "{}", line);
                assert!(!line.contains("|| []"), "{}", line);
                assert!(!line.contains("|| {}"), "{}", line);
                assert!(!line.contains("|| 0"), "{}", line);
                assert!(!line.contains("|| false"), "{}", line);
                assert!(!line.contains("|| true"), "{}", line);
            }
        }
    }
}
