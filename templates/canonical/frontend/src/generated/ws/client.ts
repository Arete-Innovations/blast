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
