//! Naming helpers for the projection-struct emitter.
//!
//! Canonical type-name shape: `<TypePascal><Variant>`.
//!
//! For a resource named `users`:
//!   - base/Db row     -> `User`
//!   - insertable      -> `UserInsertable`
//!   - patch           -> `UserPatch`
//!   - public          -> `UserPublic`
//!   - admin           -> `UserAdmin`
//!   - filter          -> `UserFilter`
//!
//! No `ForCreate` / `ForUpdate` / `Row` suffix sprawl. The variant is the
//! suffix, full stop. Wave-3 callers (`flows`, `http_routes`, `ws_topics`,
//! `vue/*`) currently use the longer form (e.g. `UserInsertableForCreate`,
//! `UserPublicRow`); aligning them to the canonical names is a follow-up
//! lane and is explicitly out of scope here.

use crate::state::FieldVariant;

/// Strip a single trailing `s` / `ies` / `xes` / etc. plural marker so that a
/// resource table named `users` yields a singular `User` projection.
///
/// This mirrors the heuristic already used by the wave-3 emitters to keep
/// the names aligned with what they expect on import.
pub fn singularize(table: &str) -> String {
    for suffix in ["sses", "shes", "ches", "xes", "zes"] {
        match table.strip_suffix(suffix) {
            Some(stem) => return format!("{}{}", stem, &suffix[..suffix.len() - 2]),
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

/// Snake/kebab → PascalCase. `user_account` -> `UserAccount`.
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

/// Pascal-cased singular type stem (`users` -> `User`).
pub fn type_stem(table: &str) -> String {
    pascal_case(&singularize(table))
}

/// Suffix appended to the type stem for each variant. `Db` is the
/// unsuffixed base struct (the "row" type that backs Diesel `Queryable`);
/// every other variant gets its own suffix.
pub fn variant_suffix(variant: FieldVariant) -> &'static str {
    match variant {
        FieldVariant::Db => "",
        FieldVariant::Insertable => "Insertable",
        FieldVariant::Patch => "Patch",
        FieldVariant::Public => "Public",
        FieldVariant::Admin => "Admin",
    }
}

/// Suffix used for the dedicated `Filter` projection (List endpoint query
/// shape). Not a `FieldVariant`; derived from `ListOptions.filterable_columns`.
pub const FILTER_SUFFIX: &str = "Filter";

/// Final emitted struct name for a given variant, e.g.
/// (`users`, `Insertable`) -> `UserInsertable`.
pub fn struct_name_for_variant(table: &str, variant: FieldVariant) -> String {
    format!("{}{}", type_stem(table), variant_suffix(variant))
}

/// Final emitted struct name for the filter projection, e.g.
/// `users` -> `UserFilter`.
pub fn filter_struct_name(table: &str) -> String {
    format!("{}{}", type_stem(table), FILTER_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(pascal_case("user"), "User");
        assert_eq!(pascal_case("user_account"), "UserAccount");
        assert_eq!(pascal_case("two_factor_auth"), "TwoFactorAuth");
    }

    #[test]
    fn type_stem_combines_singular_and_pascal() {
        assert_eq!(type_stem("users"), "User");
        assert_eq!(type_stem("user_accounts"), "UserAccount");
        assert_eq!(type_stem("companies"), "Company");
    }

    #[test]
    fn variant_suffix_matches_canonical_names() {
        assert_eq!(variant_suffix(FieldVariant::Db), "");
        assert_eq!(variant_suffix(FieldVariant::Insertable), "Insertable");
        assert_eq!(variant_suffix(FieldVariant::Patch), "Patch");
        assert_eq!(variant_suffix(FieldVariant::Public), "Public");
        assert_eq!(variant_suffix(FieldVariant::Admin), "Admin");
    }

    #[test]
    fn struct_name_for_variant_drops_suffix_for_db() {
        assert_eq!(
            struct_name_for_variant("users", FieldVariant::Db),
            "User"
        );
        assert_eq!(
            struct_name_for_variant("users", FieldVariant::Insertable),
            "UserInsertable"
        );
        assert_eq!(
            struct_name_for_variant("users", FieldVariant::Patch),
            "UserPatch"
        );
        assert_eq!(
            struct_name_for_variant("users", FieldVariant::Public),
            "UserPublic"
        );
        assert_eq!(
            struct_name_for_variant("users", FieldVariant::Admin),
            "UserAdmin"
        );
    }

    #[test]
    fn filter_struct_name_uses_filter_suffix() {
        assert_eq!(filter_struct_name("users"), "UserFilter");
        assert_eq!(filter_struct_name("user_accounts"), "UserAccountFilter");
    }
}
