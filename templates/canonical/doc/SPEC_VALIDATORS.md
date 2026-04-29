# SPEC_VALIDATORS

Single-source field validation. Rules declared once in Primer (`storage/blast/state/resources/<name>.ron`) → identical-semantics validators codegen'd to Rust route handlers AND TypeScript API clients. No external libs. No drift.

## Why

Hand-rolling validation in two languages drifts. The day someone tightens the email regex on the FE but forgets the backend, an attacker (or a curl-wielding QA) bypasses the FE and the backend accepts garbage. Catablast's posture: **rule lives in Primer, codegen emits to both sides, both share the same regex string and bound checks.**

The validators pass is parallel to enums (`SPEC_STATE.md` § "Postgres ENUM end-to-end"): SQL-first / state-first declaration + bidirectional codegen.

## Rule set (LOCKED)

`crate::state::resource::ValidatorRule` (defined in `blast/src/state/resource.rs`):

| Variant | RON syntax | Rust effect | TS effect |
|---------|-----------|-------------|-----------|
| `Required` | `Required` | `if value.is_empty() return ValidationFailed{ field, "required" }` | `if (!value || value.length === 0)` |
| `MinLen(n)` | `MinLen(8)` | `if v.len() < n return ValidationFailed{ field, "min_len" }` | `if (value.length < n)` |
| `MaxLen(n)` | `MaxLen(254)` | `if v.len() > n` | `if (value.length > n)` |
| `MinValue(n)` | `MinValue(0)` | `if v < n` (numeric) | `if (value < n)` |
| `MaxValue(n)` | `MaxValue(150)` | `if v > n` | `if (value > n)` |
| `Pattern(re)` | `Pattern("^[a-z]+$")` | `if !regex.is_match(v)` (lazy_static + `regex` crate) | `if (!new RegExp(re).test(v))` |
| `OneOf([...])` | `OneOf(["a","b","c"])` | `if !["a","b","c"].contains(&v)` | `if (!["a","b","c"].includes(v))` |
| `Email` | `Email` | shorthand for `Pattern("^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$")` | same regex, same string |
| `Url` | `Url` | shorthand for a fixed http(s) URL regex | same regex, same string |

Stable. New rules require schema_version bump + upgrader.

## Where rules live

```ron
FieldState(
    column: "email",
    variants: [DB, Insertable, Patch, Public, Admin],
    validators: [Email, MaxLen(254)],
),
FieldState(
    column: "title",
    variants: [DB, Insertable, Patch, Public],
    validators: [Required, MinLen(1), MaxLen(200)],
),
FieldState(
    column: "age",
    variants: [DB, Insertable, Patch, Public],
    nullable: true,
    validators: [MinValue(0), MaxValue(150)],
),
```

`validators` is `BTreeSet<ValidatorRule>` so order doesn't matter; codegen emits checks in stable order (enum-discriminant order).

## Codegen output

### Rust (`src/structs/generated/validators/<r>.rs`)

```rust
use crate::meltdown::MeltDown;
use crate::structs::generated::<r>::{<R>Insertable, <R>Patch};

pub fn validate_<r>_insertable(input: &<R>Insertable) -> Result<(), MeltDown> {
    // Field: email
    if !EMAIL_RE.is_match(&input.email) {
        return Err(MeltDown::validation_failed("email", "must be a valid email"));
    }
    if input.email.len() > 254 {
        return Err(MeltDown::validation_failed("email", "must be at most 254 chars"));
    }
    // Field: title
    if input.title.is_empty() {
        return Err(MeltDown::validation_failed("title", "required"));
    }
    // ... etc
    Ok(())
}

pub fn validate_<r>_patch(input: &<R>Patch) -> Result<(), MeltDown> {
    // Same checks but each field wrapped in `if let Some(v) = &input.field`
}
```

`MeltDown::validation_failed(field, msg)` is a new constructor (or extend `MeltDown::ValidationFailed { field_errors: HashMap<String, String> }` to support multi-field reporting; pick one shape and lock it).

Static regex via `once_cell::sync::Lazy<Regex>` at module top.

### TypeScript (`frontend/src/generated/validators/<r>.ts`)

```ts
import type { <R>Insertable, <R>Patch } from '@/generated/types/<r>';

export type FieldErrors = Record<string, string>;

export function validate<R>Insertable(input: <R>Insertable): FieldErrors | null {
  const errors: FieldErrors = {};
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(input.email)) {
    errors.email = 'must be a valid email';
  } else if (input.email.length > 254) {
    errors.email = 'must be at most 254 chars';
  }
  if (input.title.length === 0) {
    errors.title = 'required';
  }
  // ... etc
  return Object.keys(errors).length === 0 ? null : errors;
}

export function validate<R>Patch(input: <R>Patch): FieldErrors | null { ... }
```

TS validators return `null` for valid (cheaper than empty-object branch), or a flat `Record<string, string>` mapping field name → first error message.

**Why first-error-per-field instead of array of errors per field:** keeps the FE wire shape simple. PrimeVue form inputs only display one error string per field anyway; reporting all 5 reasons an email is invalid is UX noise.

## Wire-in points

### Backend (route handlers)

Generated `transport/http/generated/<r>.rs` create/update calls validator BEFORE flow:

```rust
async fn create(State(ctx): State<Ctx>, Json(input): Json<<R>Insertable>) -> Result<...> {
    validate_<r>_insertable(&input)?;
    Ok(Json(flows::generated::<r>::create::run(&ctx, input).await?))
}
```

The `?` propagates `MeltDown::ValidationFailed` which has `IntoResponse` → 400 + JSON `{ error: { type: "ValidationFailed", field, message } }`.

### Frontend (API clients)

Generated `frontend/src/generated/api/<r>.ts` mutation calls validator BEFORE fetch:

```ts
export async function create<R>(input: <R>Insertable): Promise<<R>Result> {
  const errors = validate<R>Insertable(input);
  if (errors !== null) {
    return { error: { type: 'ValidationFailed', field_errors: errors }, data: null };
  }
  const response = await fetch('/api/<r>', { method: 'POST', body: JSON.stringify(input), ... });
  // ... rest
}
```

Cuts a server roundtrip for any locally-detectable bad input. Same wire shape (`MeltDownResponse`-flavored error) so FE display code doesn't branch on synthetic vs real.

### Generated forms (Vue)

Generated `frontend/src/components/generated/forms/<r>/CreateForm.vue` consumes the validator reactively:

```vue
<script setup lang="ts">
import { computed, ref } from 'vue';
import { validate<R>Insertable } from '@/generated/validators/<r>';
import { useCreate<R> } from '@/generated/composables/<r>';

const form = ref<<R>Insertable>({ /* defaults */ });
const errors = computed(() => validate<R>Insertable(form.value) ?? {});
const valid = computed(() => Object.keys(errors.value).length === 0);

const create = useCreate<R>();
async function submit() {
  if (!valid.value) return;
  const { data, error } = await create(form.value);
  if (error) { /* server still rejected — display */ }
  else { /* success */ }
}
</script>
```

Codegen'd forms get this shape automatically. Hand-written forms (canonical's `LoginPage.vue`, `RegisterPage.vue`) consume the same helpers.

## Cross-field validation

Out of scope for the per-field rules. If a use case needs `password === confirm_password` or `start_date < end_date`, the check goes in:
- **Frontend:** form's `<script setup>` as a custom `computed`. The codegen'd validator handles per-field; the form layers cross-field on top.
- **Backend:** the corresponding routine (`routines/<resource>/<verb>.rs`) — routines own business invariants beyond field-level shape.

Don't extend `ValidatorRule` with a `Cross(fn_name)` escape hatch. That tries to express in declarative state what is naturally code; defer to the layer above.

## Regex compatibility

Rust `regex` crate is RE2-based (no lookahead, no backreferences). JS RegExp supports those. To avoid drift, codegen restricts emitted patterns to the **intersection**:
- Character classes, repetition, alternation, anchors: same syntax both sides.
- No lookahead `(?=...)`, no lookbehind `(?<=...)`, no backreferences `\1`.
- Catablast's bundled `Email` and `Url` shortcuts use ONLY the intersection.

If the user writes a `Pattern(...)` rule using JS-only features, **the Rust validator will reject the regex at compile time** (lazy_static panic on first use). Build-time: `blast gen validators` could pre-compile and surface the error before scaffolding ships, but that's a follow-up.

## Pipeline placement

```
schema → enums → structs → models → routines → flows → http_routes →
  frontend_types → frontend_api → composables → validators → components → pages →
  theme → icons → env_example → governor_plugin
```

`validators` runs after `frontend_api` (which emits the API clients that wrap the validator) and BEFORE `components` / `pages` (which import the validator into form bindings).

`gen_level` filter: `r.gen_level >= GenLevel::Types`. Validators emit alongside types/api — once a resource has TypeScript types, validators are useful even before composables/components level kicks in.

## Anti-patterns

**Trusting the FE alone:**
```rust
async fn create(Json(input): Json<UserInsertable>) -> Result<...> {
    flows::users::create::run(&ctx, input).await  // skipped validator
}
```
Banned. curl bypasses the FE; the backend MUST validate.

**Hand-rolling regex in pages:**
```ts
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const email_valid = computed(() => EMAIL_RE.test(email.value));
```
Banned (post-shipping). Use `validate<R>Insertable` or its primitives. The Login/Register canonical pages will be retrofitted to consume codegen'd validators.

**External validation libs (Zod, Yup, Valibot, validator, garde):**
Banned. Catablast philosophy: opinionated, no extra deps, codegen owns the shape. The minimal rule set covers the 80/20 of real CRUD forms; everything beyond goes into routines (cross-field) or the form's `<script setup>` (UI-only constraints).

## Related specs

- `blast/doc/SPEC_STATE.md` — `FieldState.validators: BTreeSet<ValidatorRule>` schema location
- `blast/doc/SPEC_CODEGEN.md` — pipeline order, hash markers, what validators emit
- `SPEC_FRONTEND.md` — list endpoint validators (page/page_size/sort) are a different track; this spec covers FIELD-level validation on insertable/patch bodies
- `SPEC_MELTDOWN.md` — `MeltDown::ValidationFailed` variant shape + IntoResponse mapping
