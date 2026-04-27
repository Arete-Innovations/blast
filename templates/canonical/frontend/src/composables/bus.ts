type Listener<T> = (payload: T) => void;

const channels = new Map<string, Set<Listener<unknown>>>();

export function emit<T>(channel: string, payload: T): void {
  const set = channels.get(channel);
  if (!set) return;
  for (const listener of set) {
    listener(payload as unknown);
  }
}

export function on<T>(channel: string, listener: Listener<T>): () => void {
  let set = channels.get(channel);
  if (!set) {
    set = new Set();
    channels.set(channel, set);
  }
  set.add(listener as Listener<unknown>);
  return () => off(channel, listener);
}

export function off<T>(channel: string, listener: Listener<T>): void {
  const set = channels.get(channel);
  if (!set) return;
  set.delete(listener as Listener<unknown>);
  if (set.size === 0) channels.delete(channel);
}

export function clearAll(): void {
  channels.clear();
}
