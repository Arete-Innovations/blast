// url — URL-state binding helpers. SPAs broke the web's URL contract;
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
