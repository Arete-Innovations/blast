// channel — low-level Relay WS subscription primitive. Per-resource
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
