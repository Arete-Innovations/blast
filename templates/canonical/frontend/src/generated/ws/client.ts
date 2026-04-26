// Auto-generated. Do not edit by hand.
// Relay WS multiplexer client. Single connection, multiple topics.
// Per SPEC_RELAY.md the FE re-fetches from DB on resubscribe ack;
// there is no replay buffer. This module exposes the consumer-facing
// surface (`subscribe<T>` / `unsubscribe`) used by the channel
// composable. Transport hookup is internal — flip implementations
// by editing this codegen, not the call sites.

type Handler<T> = (payload: T) => void

class WsClient {
  private handlers: Map<string, Set<Handler<unknown>>> = new Map()

  subscribe<T>(topic: string, handler: Handler<T>): void {
    let set = this.handlers.get(topic)
    if (set === undefined) {
      set = new Set()
      this.handlers.set(topic, set)
    }
    set.add(handler as Handler<unknown>)
  }

  unsubscribe<T>(topic: string, handler: Handler<T>): void {
    const set = this.handlers.get(topic)
    if (set === undefined) {
      return
    }
    set.delete(handler as Handler<unknown>)
    if (set.size === 0) {
      this.handlers.delete(topic)
    }
  }
}

export const wsClient = new WsClient()
