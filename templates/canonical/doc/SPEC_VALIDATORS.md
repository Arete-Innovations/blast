# SPEC_VALIDATORS

Single-source field validation. Rules declared once in Primer (`storage/blast/state/resources/<name>.ron`) → ONE Rust validator emitted. The validator runs on both the server (REST handler) and the client (Leptos form via WASM) because the whole crate compiles to both targets.

## Why single-source

The pre-leptos design emitted paired Rust + TS validators with byte-identical regex strings. Drift was a constant threat. With Leptos compiling to WASM, the same Rust validator runs in both places. **One source. No drift. Impossible by construction.**

## Rule set (LOCKED)

`crate::state::resource::ValidatorRule` (defined in `blast/src/state/resource.rs`):

| Variant | RON syntax | Effect |
|---------|-----------|--------|
| `Required` | `Required` | `if value.is_empty() return ValidationFailed{ field, "required" }` |
| `MinLen(n)` | `MinLen(8)` | `if v.len() < n return ValidationFailed{ field, "min_len" }` |
| `MaxLen(n)` | `MaxLen(254)` | `if v.len() > n` |
| `MinValue(n)` | `MinValue(0)` | `if v < n` (numeric) |
| `MaxValue(n)` | `MaxValue(150)` | `if v > n` |
| `Pattern(re)` | `Pattern("^[a-z]+$")` | `if !regex.is_match(v)` (lazy_static + `regex` crate) |
| `OneOf([...])` | `OneOf(["a","b","c"])` | `if !["a","b","c"].contains(&v)` |
| `Email` | `Email` | shorthand for `Pattern("^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$")` |
| `Url` | `Url` | shorthand for a fixed http(s) URL regex |

Stable. New rules require schema_version bump + upgrader.

## Where rules live

```ron
FieldState(
    sql_type: "Varchar",
    variants: [Db, Insertable, Patch, Public, Admin],
    validators: [Email, MaxLen(254)],
),
```

`validators` is `BTreeSet<ValidatorRule>` so order doesn't matter; codegen emits checks in stable order (enum-discriminant order).

## Codegen output

Single file per resource, **Rust only**:

```rust
// src/structs/generated/validators/<r>.rs
use crate::meltdown::MeltDown;
use crate::structs::generated::<r>::{<R>Insertable, <R>Patch};

pub fn validate_<r>_insertable(input: &<R>Insertable) -> Result<(), MeltDown> {
    // Field: email
    if !EMAIL_RE.is_match(&input.email) {
        return Err(MeltDown::validation_failed_field("email", "must be a valid email"));
    }
    if input.email.len() > 254 {
        return Err(MeltDown::validation_failed_field("email", "must be at most 254 chars"));
    }
    Ok(())
}

pub fn validate_<r>_patch(input: &<R>Patch) -> Result<(), MeltDown> { ... }
```

Static regex via `once_cell::sync::Lazy<Regex>` at module top.

## Wire-in points

### Backend (REST handlers)

Generated `transport/http/generated/api/<r>.rs` create/update calls validator BEFORE flow:

```rust
async fn create(Extension(ctx): Extension<Ctx>, Json(input): Json<<R>Insertable>) -> Result<...> {
    validate_<r>_insertable(&input)?;
    Ok(Json(flows::generated::<r>::create::run(&ctx, input).await?))
}
```

The `?` propagates `MeltDown::ValidationFailed` which has `IntoResponse` → 400 + JSON envelope.

### Frontend (Leptos forms + Action)

Generated `<R>CreateForm` runs the validator client-side BEFORE dispatching the Action:

```rust
let create_action = Action::new(|input: &<R>Insertable| {
    let input = input.clone();
    async move {
        if let Err(e) = validate_<r>_insertable(&input) {
            return Err(e);
        }
        crate::transport::leptos::data::generated::<r>::do_<r>_create(input).await
    }
});
```

Cuts a server roundtrip for any locally-detectable bad input. **Same Rust function**, no synthetic envelope, no separate FE validator. The error ends up in the `Action`'s `value()` signal and renders via `<ErrorBanner>` or per-field error binding.

## Cross-field validation

Out of scope for per-field rules. If a use case needs `password === confirm_password` or `start_date < end_date`, the check goes in:

- **Frontend**: form's `<script>` body as a `Memo`. The codegen'd validator handles per-field; the form layers cross-field on top.
- **Backend**: the corresponding routine (`routines/<resource>/<verb>.rs`) — routines own business invariants beyond field-level shape.

Don't extend `ValidatorRule` with a `Cross(fn_name)` escape hatch.

## Regex compatibility

Now that there's only one regex implementation (Rust's `regex` crate compiled to WASM), the JS RegExp compatibility constraint is gone. **You can use any RE2 feature** that the `regex` crate supports. Lookahead and backreferences still aren't supported by RE2, but the WASM regex behaves identically to the server-side regex.

## Pipeline placement

```
schema → enums → structs → validators → models → routines → flows → http_routes
       → leptos_pages → leptos_forms → leptos_tables → app_routes → env_example
```

`validators` runs after `structs` (which emits the projection types validators reference) and BEFORE the consumers (REST handlers in `http_routes`, form components in `leptos_forms`).

`gen_level` filter: `r.gen_level >= GenLevel::Types`. Validators are useful as soon as types exist.

## Hand-written code (auth)

`auth` is hand-written canonical (not driven by a Primer), so it does NOT get codegen'd validators. The hand-written Login/Register pages perform their own minimal client-side checks (non-empty fields, password match) and rely on the BE auth flow for authoritative validation. The BE returns `MeltDown` envelope, the FE renders `error.message` byte-for-byte.

## Anti-patterns

**Trusting client validation alone:**
```rust
async fn create(Json(input): Json<UserInsertable>) -> Result<...> {
    flows::users::create::run(&ctx, input).await  // skipped validator
}
```
Banned. curl bypasses the WASM; the BE handler MUST call the validator.

**Hand-rolling regex in pages:**
```rust
let email_re = regex::Regex::new(r"...").unwrap();
```
Banned. Use the codegen'd validator.

**External validation libs (validator, garde, anything Rust):**
Banned. Catablast philosophy: opinionated, no extra deps, codegen owns the shape. Cross-field lives in routines + form `Memo`s.

## Related specs

- `blast/doc/SPEC_STATE.md` — `FieldState.validators: BTreeSet<ValidatorRule>` schema location
- `blast/doc/SPEC_CODEGEN.md` — pipeline order, hash markers
- `SPEC_LEPTOS.md` — Leptos UI, where forms live
- `SPEC_MELTDOWN.md` — `MeltDown::ValidationFailed` variant shape + IntoResponse mapping
