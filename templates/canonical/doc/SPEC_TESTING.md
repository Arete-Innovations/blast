# SPEC_TESTING

Testing strategy for Catablast apps. Real Postgres, transaction rollback, Blast-scaffolded baselines. No DB mocks.

## Core Principle

Mocking the DB tests nothing useful. DB is the canonical state store — tests must hit it for real. Any test that passes against a mock but fails against Postgres is a false negative. Prior incident: mock/prod divergence masked a broken migration that reached production.

## Test DB

- **Dev:** scratch DB named `<appname>_test_<pid>` on the local Postgres instance.
- **CI:** disposable Postgres container. `DATABASE_URL_TEST` env var points at it.
- `blast test` creates the test DB, runs all migrations, runs the suite, drops the DB on exit.
- Migrations run on the test DB before every `blast test` invocation. Stale schema → test failure, not silent skip. That's the safety net working.

## Transaction Rollback Harness

Each test runs inside a Postgres transaction that rolls back on completion. ~10× faster than schema-per-test; isolation without teardown SQL.

```rust

async fn with_test_tx<F, Fut>(f: F)
where
    F: FnOnce(TestConn) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut conn = test_pool().await.get().await.unwrap();
    conn.begin_test_transaction().await.unwrap();
    f(conn).await;

}
```

Tests run **serially by default** — rollback on a shared pool is not parallel-safe. Fast enough for the target scale.

### Limits

- Tests that span multiple DB connections need a separate schema. Opt-in escape hatch: `#[blast_test(schema_per_test)]` (future attribute). Not needed for normal flows.
- Tests that verify Postgres triggers or deferred constraints may need the full transaction to commit. Handle when encountered.

## Blast-Generated Test Scaffolds

`blast gen flows` and `blast gen frontend` (route gen) emit baseline test files alongside each generated module.

### Per-flow baseline

```
flows/generated/users/list.test.rs
flows/generated/users/get.test.rs
flows/generated/users/update.test.rs
flows/generated/users/delete.test.rs
```

Each baseline test:
1. Inserts a fixture by calling the `create` flow (or a fixture helper).
2. Calls the flow under test with minimal valid input.
3. Asserts the return value shape and DB state post-call.

```rust

#[tokio::test]
async fn test_get_user_baseline() {
    with_test_tx(|conn| async move {
        let user = fixtures::create_user(&conn, Default::default()).await.unwrap();
        let result = users::get::run(&conn, user.id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, user.id);
    }).await;
}
```

User adds edge-case assertions in `flows/users/get_edge_cases.test.rs`. Blast never touches user-owned files.

### Per-route baseline

```
transport/http/generated/users.test.rs
```

Fires a request through the full middleware → handler → flow chain via `axum::ServiceExt::oneshot`. No mocked handler; real flow execution, real DB write, real response.

```rust

#[tokio::test]
async fn test_get_user_route_baseline() {
    with_test_tx(|conn| async move {
        let user = fixtures::create_user(&conn, Default::default()).await.unwrap();
        let app = create_test_app(conn.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/users/{}", user.id))
                    .header("Authorization", format!("Bearer {}", test_session_token(&conn, user.id).await))
                    .body(Body::empty()).unwrap()
            )
            .await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }).await;
}
```

## Fixtures via Flows

Test setup calls the generated `create` flow (or helper fns in `tests/fixtures/` that call flows). Raw `INSERT INTO` is banned in test setup.

```rust

pub async fn create_user(conn: &TestConn, overrides: UserOverrides) -> Result<User, MeltDown> {
    users::create::run(conn, NewUser {
        email: overrides.email.unwrap_or_else(|| format!("test-{}@example.com", Uuid::new_v4())),
        password: overrides.password.unwrap_or_else(|| "testpassword123".to_string()),
        ..Default::default()
    }).await
}
```

Fixture helpers are thin. They delegate to flows. If a flow is broken, fixtures break first — loud signal before the actual test asserts anything.

Fixtures live in `tests/fixtures/<resource>.rs`. Blast scaffolds a stub fixture module when it scaffolds the `create` flow.

## Unit Tests

Pure functions in `routines/derive/` use `#[cfg(test)]` in-module unit tests. No DB, no async.

Routines in `routines/act/` and `routines/collect/` call models and services — they need a test conn and run as integration tests inside `with_test_tx`, same as flow tests. Blast scaffolds a `routines/generated/<resource>.test.rs` baseline alongside each generated routine module.

Services:
- **Crypto / hashing (pure):** unit tests in-module.
- **Email:** test-double impl (`InMemoryMailer`) that captures sent messages; tests assert against the captured outbox.
- **Storage:** `TmpDirStorage` impl backed by a temp directory; no network.
- **Rate-limit:** in-memory impl (already is in production; no swap needed).

Everything touching the DB → integration test, no exceptions.

## Command Surface

```
blast test                   # run full suite (creates test DB, migrates, runs, drops)
blast test <filter>          # run matching tests (cargo test -- <filter>)
blast test --no-drop         # keep test DB after run (inspect state)
```

Under the hood: `cargo test` with `DATABASE_URL_TEST` set. Test DB management wrapped around `cargo test` invocation.

## CI Recipe

```yaml
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_PASSWORD: test
      POSTGRES_DB: app_test
    ports: ["5432:5432"]

env:
  DATABASE_URL_TEST: postgres://postgres:test@localhost/app_test

steps:
  - run: blast test
```

No schema import needed — `blast test` runs all migrations fresh before the suite.

## Anti-Patterns

**Mocking the DB layer:**
```rust

let mock_repo = MockUserRepo::new();
mock_repo.expect_get().returning(|_| Ok(fake_user()));
```
Mock passes, Postgres fails. Use real DB in transaction.

**`INSERT INTO` in fixtures:**
```rust

diesel::insert_into(users::table).values(&test_row).execute(&conn)?;
```
Bypasses flows, skips validation, doesn't fire WS events. Use `fixtures::create_user(...)`.

**Sharing DB state between tests:**
Each test gets its own transaction. Never share a transaction across tests; the rollback boundary is the test.

## Related Specs

- `SPEC_FLOWS.md` — flow shape; flows call routines (not models/services directly); tests call flows directly
- `SPEC_ROUTINES.md` — routine shape; routines call models + services; routine integration tests use `with_test_tx`
- `SPEC_MELTDOWN.md` — error type returned by flows; assert on `MeltDown` variants
- `SPEC_SERVICES.md` — service test-double implementations
- `blast/doc/SPEC_CODEGEN.md` — `.test.rs` scaffold generation pass
- `blast/doc/SPEC_BLAST_COMMANDS.md` — `blast test` command
