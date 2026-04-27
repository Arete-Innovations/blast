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
