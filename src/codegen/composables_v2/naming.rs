//! Naming helpers for composables_v2.
//!
//! The Rust struct/type naming is delegated to
//! `crate::codegen::structs::naming` so emitted TS imports line up with
//! the generated types module. The TS-side composable identifiers are
//! camelCase by FE-framework convention (`useUsersList`,
//! `useCreateUser`); the BE wire types stay snake_case as per Governor's
//! `SnakeCaseInterfaceFields` rule.

use crate::codegen::structs::naming::type_stem_for_resource;
use crate::state::ResourceState;

/// Singular type stem honouring `singular_override`. Equivalent to
/// `User` for `users`, `Datum` for `data` (override).
pub fn singular_pascal(resource: &ResourceState) -> String {
    type_stem_for_resource(resource)
}

/// Plural Pascal stem used in identifiers like `useUsersList`. The
/// table name is already plural; we PascalCase it without going through
/// Inflector (which would re-singularize `users` → `user`). Splits on
/// `_` and `-` boundaries and uppercases the first letter of each
/// segment.
pub fn plural_pascal(table: &str) -> String {
    let mut out = String::with_capacity(table.len());
    let mut upper_next = true;
    for ch in table.chars() {
        if ch == '_' || ch == '-' {
            upper_next = true;
            continue;
        }
        if upper_next {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Bus event prefix — keyed off the table name, matching the WS topic
/// convention (`<table>:created`). Keeping it the table name (not the
/// singular) avoids `user:created` ambiguity with per-row `users:42`
/// topic style.
pub fn bus_prefix(table: &str) -> String {
    table.to_string()
}

/// Emitted file name (no extension): one composable file per resource,
/// keyed by the table name.
pub fn file_stem(table: &str) -> String {
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::ResourceName;

    #[test]
    fn singular_pascal_handles_basic_table() {
        let r = ResourceState::new(ResourceName::new("users"));
        assert_eq!(singular_pascal(&r), "User");
    }

    #[test]
    fn singular_pascal_honours_override() {
        let mut r = ResourceState::new(ResourceName::new("data"));
        r.singular_override = Some("datum".to_string());
        assert_eq!(singular_pascal(&r), "Datum");
    }

    #[test]
    fn plural_pascal_pascalizes_table_name() {
        assert_eq!(plural_pascal("users"), "Users");
        assert_eq!(plural_pascal("user_accounts"), "UserAccounts");
    }

    #[test]
    fn bus_prefix_uses_table_name() {
        assert_eq!(bus_prefix("users"), "users");
    }

    #[test]
    fn file_stem_uses_table_name() {
        assert_eq!(file_stem("users"), "users");
    }
}
