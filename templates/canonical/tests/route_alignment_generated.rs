// AUTO-GENERATED from storage/blast/state/app.ron @ c196c9b8f01e3089f6efceef8e0b434aa0652e9351ec4f2f7b42fca54207b6f1
//
// Do not edit by hand. Run `blast gen all` after mutating state.

//! Asserts every generated `path!(...)` literal equals the canonical
//! `RouteName::*` enum path for the same resource. Drift between the macro
//! literal in `src/transport/leptos/routes/generated/table.rs` and the enum
//! constructor would cause silent breakage at navigation time — this test
//! is the compile-time backstop.

use canonical::structs::leptos::RouteName;

#[test]
fn no_generated_routes() {
    // Intentionally empty — no resources at gen_level >= Pages.
}
