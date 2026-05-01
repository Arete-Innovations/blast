//! Naming helpers for the model emitter.
//!
//! Mirrors the convention already locked by the structs emitter:
//!
//! - resource table `users` -> base type stem `User`
//! - generated query types: `UserQuery`, `UserQueryPaginated`
//! - sort enum (assumed pre-emitted by structs codegen as the Sort variant)
//! - filter enum (assumed pre-emitted by structs codegen as the Filter variant)
//!
//! Kept as a thin separate module so the model codegen can swap in
//! the `singular_override` field once `state-extensions` lands without
//! rippling changes through the rest of the crate.

use crate::state::ResourceState;

pub fn singularize(table: &str) -> String {
    for suffix in ["sses", "shes", "ches", "xes", "zes"] {
        match table.strip_suffix(suffix) {
            Some(stem) => {
                return format!("{}{}", stem, &suffix[..suffix.len() - 2]);
            }
            None => continue,
        }
    }
    match table.strip_suffix("ies") {
        Some(stem) => format!("{}y", stem),
        None => match table.strip_suffix('s') {
            Some(stem) => stem.to_string(),
            None => table.to_string(),
        },
    }
}

pub fn pascal_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut upper_next = true;
    for ch in input.chars() {
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

/// `users` -> `User`. Honors `singular_override` when state-extensions adds
/// the field; today this is just a heuristic singularize + Pascal-case.
pub fn type_stem_for(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    pascal_case(&singularize(table))
}

/// `<Type>Query` — fluent builder type name.
pub fn query_type(stem: &str) -> String {
    format!("{stem}Query")
}

/// `<Type>QueryPaginated` — terminal builder type name for paginated queries.
pub fn query_paginated_type(stem: &str) -> String {
    format!("{stem}QueryPaginated")
}

/// `<Type>Insertable` — sibling struct emitted by `codegen::structs`.
pub fn insertable_type(stem: &str) -> String {
    format!("{stem}Insertable")
}

/// `<Type>Patch` — sibling struct emitted by `codegen::structs`.
pub fn patch_type(stem: &str) -> String {
    format!("{stem}Patch")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::ResourceName;

    #[test]
    fn singularize_handles_common_endings() {
        assert_eq!(singularize("users"), "user");
        assert_eq!(singularize("addresses"), "address");
        assert_eq!(singularize("companies"), "company");
        assert_eq!(singularize("boxes"), "box");
        assert_eq!(singularize("media"), "media");
    }

    #[test]
    fn pascal_case_handles_snake() {
        assert_eq!(pascal_case("user_account"), "UserAccount");
        assert_eq!(pascal_case("two_factor_auth"), "TwoFactorAuth");
    }

    #[test]
    fn type_stem_combines() {
        let r = ResourceState::new(ResourceName::new("user_accounts"));
        assert_eq!(type_stem_for(&r), "UserAccount");
    }

    #[test]
    fn builder_type_names() {
        assert_eq!(query_type("User"), "UserQuery");
        assert_eq!(query_paginated_type("User"), "UserQueryPaginated");
        assert_eq!(insertable_type("User"), "UserInsertable");
        assert_eq!(patch_type("User"), "UserPatch");
    }
}
