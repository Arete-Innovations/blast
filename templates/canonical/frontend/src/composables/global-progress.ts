// global-progress — module-scoped active-task counter backing the
// GlobalProgressBar. Multiple concurrent navs/fetches each call start();
// finish() decrements; isActive is true when count > 0. cancel() force-
// clears (used on hard router error). No external state library — this
// IS the state. See SPEC_FRONTEND_ROUTING.md.

import { computed, ref } from 'vue'
import type { ComputedRef } from 'vue'

const active_count = ref(0)
const is_active = computed<boolean>(() => active_count.value > 0)

export interface GlobalProgressHandle {
  isActive: ComputedRef<boolean>
  start: () => void
  finish: () => void
  cancel: () => void
}

export function useGlobalProgress(): GlobalProgressHandle {
  return {
    isActive: is_active,
    start,
    finish,
    cancel,
  }
}

function start(): void {
  active_count.value += 1
}

function finish(): void {
  if (active_count.value > 0) {
    active_count.value -= 1
  }
}

function cancel(): void {
  active_count.value = 0
}
