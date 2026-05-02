//! Dedicated emitter for the auth domain.
//!
//! Auth verbs (login/register/logout/me) are FIXED by the framework — not
//! user-defined CRUD verbs (List/Get/Create/Update/Delete). The existing
//! per-resource emitters expect primer-declared verbs; auth needs its own
//! emit path with fixed verb names and special-cased target paths under
//! `auth/` subdirs.
//!
//! This emitter always runs (auth is on by default in the framework) and
//! emits byte-stable templates for:
//!
//! - `src/structs/generated/users.rs` (User, NewUser, UserPublic — flat)
//! - `src/structs/generated/auth/{login,register,mod}.rs` (auth DTOs)
//! - `src/models/generated/users.rs` (User CRUD ops — flat)
//! - `src/routines/generated/auth/{login,register,logout,me,mod}.rs`
//! - `src/flows/generated/auth/{login,register,logout,me,mod}.rs`
//! - `src/transport/http/generated/auth.rs` (single file with all handlers)
//! - `src/transport/leptos/pages/generated/{login,register,logout,profile}.rs`
//! - `src/transport/leptos/pages/generated/profile.module.scss`
//!
//! After emitting, the runner idempotently extends the parent barrel
//! `mod.rs` files emitted by the resource-driven passes (structs, models,
//! routines, flows, http_routes, leptos_pages) so the auth submodules are
//! pulled into the crate's module tree.

pub mod runner;
pub mod templates;

pub use runner::run;
