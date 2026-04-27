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
