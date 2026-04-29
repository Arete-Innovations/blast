# SPEC_RELAY

WebSocket multiplexer + pub/sub. Lives in `crate::transport::ws`. One WS connection per authenticated session, all topics multiplexed over it.

## Why Multiplex

Opening one WS per resource subscription (orders, users, presence) creates N sockets per page, N TCP connections, N reconnect loops. Multiplexed single socket: one connection per session, subscribes dynamically to any topic. Shared reconnect logic.

## Protocol

```
client → server: {"op": "subscribe",   "topic": "orders:customer:42"}
client → server: {"op": "unsubscribe", "topic": "orders:customer:42"}
client → server: {"op": "ping"}

server → client: {"topic": "orders:customer:42", "event": {"type": "Changed", "row": {...}}}
server → client: {"op": "ack", "topic": "orders:customer:42"}
server → client: {"op": "error", "topic": "...", "reason": "forbidden"}
server → client: {"op": "pong"}
```

Topics are string-valued: `{resource}:{scope_kind}:{scope_value}`. Examples:
- `orders:customer:42`
- `presence:team:all`
- `users:id:7`

Topic grammar is determined by WS event declarations in the resource state file; user code doesn't build raw topic strings by hand — typed helpers construct them.

## Connection Lifecycle

```
1. Client opens WS at /ws with auth (cookie or Authorization: Bearer)
2. Server validates session (via session middleware); attaches user context
3. Client sends subscribe messages for the topics it wants
4. Server per subscription: runs can_subscribe() auth check for that topic
5. On DB write matching a subscribed topic, model calls relay::publish after commit
6. On unsubscribe or disconnect, server removes subscriptions and cleans up
7. Server keeps session_id → HashSet<topic> in memory
8. Periodic ping/pong keeps connection alive; auto-reconnect in FE client
```

WS message handlers are transport. Per the layer contract, they call one flow per message — never models, routines, or services directly. The flow declares the `Crank` retry policy for the operation and delegates work to routines; routines call models and services.

## Subscription Authorization

Per WS-enabled primer, user writes a `can_subscribe` impl:

```rust


impl WsAuth for OrderTopic {
    async fn can_subscribe(ctx: &Ctx, topic: &OrderTopic) -> bool {
        match topic {
            OrderTopic::Customer { customer_id } => {
                ctx.session.user_id == *customer_id || ctx.session.is_admin()
            }
        }
    }
}
```

`Ctx` is the universal session handle threaded through every layer. The WS handler receives it on upgrade and passes it into `can_subscribe`; no direct session-store lookup needed here.

- If `can_subscribe` returns true, server adds the subscription and emits `{"op":"ack"}`
- If false, server emits `{"op":"error","reason":"forbidden"}` and doesn't add the subscription

Blast scaffolds a stub for each WS-enabled resource (declared in the resource state file). User fills in the auth logic. This is the only manual step per WS-enabled resource.

## Publish Driver

Two supported drivers, per-primer:

### App-layer (default)

Generated model writes call `relay::publish` after successful DB commit:

```rust

pub async fn update_status(conn: &mut Conn, id: i64, status: OrderStatus) -> Result<Order, MeltDown> {
    let updated = diesel::update(...).execute_and_fetch(conn).await?;
    relay::channel::<OrderEvent>(&format!("orders:customer:{}", updated.customer_id))
        .publish(OrderEvent::Changed(updated.clone().into_public()));
    Ok(updated)
}
```

Simple. Fast. Only fires when writes go through the model layer. Changes from outside the app (raw SQL, migrations, admin consoles) won't publish.

The call chain is: flow (with Crank retry policy) → routine → model → `relay::publish`. The model is the correct publish site because it holds the DB result and the post-commit moment. Flows and routines do not call `relay::publish` directly.

### Postgres LISTEN/NOTIFY (opt-in)

For resources where changes come from outside the app layer, declare `ws_driver: "postgres"` in the resource state file. Blast generates Postgres triggers on the resource's table that `pg_notify` on change. Catalyst process opens a LISTEN connection and routes notifies to relay channels.

Heavier infrastructure. Catches ALL changes regardless of source. Use when the app isn't the sole writer.

## Channel Primitive

```rust

pub struct Channel<T: Serialize + Clone + Send + Sync + 'static> {
    topic: String,
    _phantom: PhantomData<T>,
}

impl<T> Channel<T> {
    pub fn publish(&self, event: T);
    pub fn subscribers_count(&self) -> usize;
}

pub fn channel<T>(topic: impl Into<String>) -> Channel<T>;
```

Internally backed by tokio broadcast channels. Topic → `broadcast::Sender<T>` map kept in the `Relay` app-state singleton.

## Per-Session State

```rust
pub struct WsSession {
    pub ctx: Ctx,
    pub subscriptions: HashSet<String>,
    pub sender: mpsc::Sender<WsMessage>,
}
```

Session state is held per-connection while the WS is open. When the WS closes, subscriptions are cleaned up.

## Reconnect Behavior (client-side)

Frontend `WsClient` singleton:
- Auto-reconnect with exponential backoff on disconnect
- On reconnect, re-subscribes to all topics that were active before
- Ping/pong every 30s to detect dead connections
- Exposes `.subscribe(topic, handler)` and `.unsubscribe(topic)` for composables

Generated composables (`useOrders({ live: true })`) call `WsClient.subscribe()` on mount and `unsubscribe()` on unmount. One socket, many topics.

### Reconnect via DB

No in-memory replay buffer. The server does not record missed events while the client was disconnected. There is no event log to drain on reconnect.

Protocol on reconnect:

```
1. WsClient detects disconnect (socket error or ping timeout)
2. WsClient begins exponential backoff (initial 500ms, cap 30s)
3. WsClient opens a new WS connection, re-authenticates via session cookie/bearer
4. WsClient re-sends subscribe messages for all previously-active topics
5. Server responds to each subscribe with {"op":"ack","topic":"..."} after
   running can_subscribe() auth check
6. On receiving ack, composable re-fetches current state via HTTP from DB
7. Composable replaces its local reactive data with the fresh DB snapshot
8. Subsequent events from this point forward update the composable normally
```

The re-fetch in step 6 is mandatory. It closes the gap that opened during the disconnected window. DB is authoritative; the reconnect path is identical to initial load.

**Why no replay buffer:**
- Fewer failure modes. No replay logic, no buffer overflow, no ordering ambiguity.
- "FE state is a war crime" rule applies: replaying events to reconstruct state is client-side state accumulation in disguise.
- The re-fetch is cheap. At the scale this stack targets (≤10K users, single VPS), a DB round-trip on reconnect is not a bottleneck.

**Consequence:** if a client was offline for 10 minutes and 50 rows changed, it sees all 50 in the re-fetch. It does not receive 50 individual events. This is correct; the DB reflects the current truth.

## FE Reconciliation Pattern

Per `DB > memory > FE state` rule, WS events do not drive client state directly. Two allowed patterns:

### Pattern A: re-fetch on event
WS event says "orders:customer:42 changed" → composable re-GETs `/api/orders?customer_id=42`. DB is authoritative. Simplest, most reliable.

### Pattern B: full-row push
WS event payload is the entire `OrderPublic` row. Composable replaces its local copy. Still DB-sourced; just skips the round-trip.

**Not allowed:** partial payloads like `{"op":"delete","id":42}` where the FE infers the DB state by mutating local arrays. That's FE-state-driven; banned.

The resource state file chooses the payload mode (`ws_payload: FullPublicRow` or `ws_payload: IdOnly`). Blast generates the composable accordingly.

## Typed Events

Each WS-enabled resource state file produces a typed event enum:

```rust

pub enum OrderEvent {
    Changed(OrderPublic),
    Deleted { id: i64 },
}
```

```ts

export type OrderEvent =
  | { type: "Changed"; row: OrderPublic }
  | { type: "Deleted"; id: number };
```

Typed both sides. No stringly-typed event dispatch.

## Topic Scope

The resource state file declares how topics partition (e.g. `ws_scope: TopicParam("customer_id")`). Options:
- `TopicParam("field")` — topic = `"orders:customer:{field_value}"`
- `Global` — topic = `"orders:all"`
- `UserId` — topic = `"orders:user:{session.user_id}"`
- `RoleGated(["admin"])` — topic = `"orders:admin"`

Dictates what topics exist and who can subscribe (via the `can_subscribe` handler).

## Anti-Patterns

**WS handler calling models or services directly:**
```rust
// ❌ wrong — transport must not reach past flows
async fn handle_message(ctx: &Ctx, msg: WsMsg) {
    models::orders::update_status(&mut conn, msg.id, msg.status).await?;
}
```

WS handlers are transport. One message → one flow call. The flow owns the `Crank` retry policy and delegates to routines; routines call models and services. The handler owns nothing except dispatch.

**Publishing events FE state should recompute:**
```rust

relay::publish(ChatEvent::UserTyping { user_id })
```

Typing indicators are ephemeral and not in DB. Belongs to a different pattern (like Presence), not Relay's contract-backed flow.

**Subscribing without auth check:**
```rust

```

The compiler requires a `WsAuth` impl per topic type before registration. Blast scaffolds a stub; user must fill it in.

**Many sockets per page:**
```ts

const ws1 = new WebSocket("/ws/orders");
const ws2 = new WebSocket("/ws/users");
```

One `WsClient` singleton. Subscribe to many topics over one socket.

**Multi-process without Postgres driver:**
If the app runs multiple Catalyst processes (horizontal scale), app-layer publish only reaches subscribers on the same process. Switch those resources' primer to `WsDriver::Postgres` or embrace the single-process assumption.

## Related Specs

- `SPEC_CONFIG.md` — resource state file format (WS events declared per resource)
- `SPEC_SESSIONS.md` — WS upgrade auth flow
- `SPEC_FRONTEND.md` — shared WsClient, composable integration
- `SPEC_ARCHITECTURE.md` — `transport/ws/` layer placement
- `blast/doc/SPEC_CODEGEN.md` — Blast generates WS event enums from resource state
