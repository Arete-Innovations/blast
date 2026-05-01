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
//!   - sort            -> `UserSort`
//!
//! Singularization is delegated to the `Inflector` crate
//! (`to_singular` + `to_class_case`). Resources may carry a
//! `singular_override: Option<String>` in state for the cases Inflector
//! gets wrong (`staff` -> `staff`, `data` -> `datum`, etc.).
//!
//! No `ForCreate` / `ForUpdate` / `Row` suffix sprawl. The variant is the
//! suffix, full stop. Wave-3 callers (`flows`, `http_routes`, `ws_topics`,
//! `vue/*`) currently use the longer form (e.g. `UserInsertableForCreate`,
//! `UserPublicRow`); aligning them to the canonical names is a follow-up
//! lane and is explicitly out of scope here.

use inflector::{cases::classcase::to_class_case, string::singularize::to_singular};

use crate::state::{FieldVariant, ResourceState};

/// English singularization via Inflector. `users` -> `user`,
/// `categories` -> `category`, `addresses` -> `address`.
///
/// Inflector's built-in irregular table is incomplete; we patch the
/// known gaps here. Override at the resource level
/// (`singular_override`) for any remaining cases Inflector gets wrong.
pub fn singularize(table: &str) -> String {
    // Inflector does not handle these common English irregulars.
    match table {
        "people" => return "person".to_string(),
        "children" => return "child".to_string(),
        "men" => return "man".to_string(),
        "women" => return "woman".to_string(),
        "teeth" => return "tooth".to_string(),
        "feet" => return "foot".to_string(),
        "mice" => return "mouse".to_string(),
        "geese" => return "goose".to_string(),
        _ => {}
    }
    to_singular(table)
}

/// Snake/kebab → PascalCase via Inflector's class-case conversion.
/// `user_account` -> `UserAccount`.
pub fn pascal_case(input: &str) -> String {
    to_class_case(input)
}

/// Pascal-cased singular type stem (`users` -> `User`).
///
/// Pure-string variant: takes only the table name. Use
/// [`type_stem_for_resource`] when a `ResourceState` is in hand so the
/// optional `singular_override` is honored.
pub fn type_stem(table: &str) -> String {
    pascal_case(&singularize(table))
}

/// Pascal-cased singular type stem honoring the resource's optional
/// `singular_override`. The override is interpreted as the *singular
/// snake/kebab form* (`datum`, `staff_member`, `octopus`); it is then
/// run through `pascal_case` so callers don't have to know whether the
/// override is already cased correctly.
pub fn type_stem_for_resource(resource: &ResourceState) -> String {
    match resource.singular_override.as_deref() {
        Some(singular) if !singular.is_empty() => pascal_case(singular),
        _absent_or_empty => type_stem(resource.name.as_str()),
    }
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

/// Suffix used for the per-resource `Sort` enum (List endpoint query
/// shape). Not a `FieldVariant`; derived from `ListOptions.sortable_columns`.
pub const SORT_SUFFIX: &str = "Sort";

/// Final emitted struct name for a given variant, honoring the
/// resource's optional `singular_override`.
pub fn struct_name_for_variant_resource(resource: &ResourceState, variant: FieldVariant) -> String {
    format!("{}{}", type_stem_for_resource(resource), variant_suffix(variant))
}

/// Final emitted struct name for the filter projection, honoring
/// `singular_override`.
pub fn filter_struct_name_for_resource(resource: &ResourceState) -> String {
    format!("{}{}", type_stem_for_resource(resource), FILTER_SUFFIX)
}

/// Final emitted enum name for the sort projection, honoring
/// `singular_override`.
pub fn sort_enum_name_for_resource(resource: &ResourceState) -> String {
    format!("{}{}", type_stem_for_resource(resource), SORT_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::ResourceName;

    #[test]
    fn singularize_handles_common_endings() {
        assert_eq!(singularize("users"), "user");
        assert_eq!(singularize("addresses"), "address");
        assert_eq!(singularize("categories"), "category");
        assert_eq!(singularize("boxes"), "box");
    }

    #[test]
    fn singularize_handles_irregulars_via_inflector() {
        // Inflector knows the canonical irregulars; pin the ones we
        // care about for resource naming so a future Inflector bump
        // can't silently regress.
        assert_eq!(singularize("people"), "person");
        assert_eq!(singularize("children"), "child");
    }

    #[test]
    fn singularize_uncountable_stays_unchanged() {
        // `staff` is uncountable in English. Inflector returns it
        // verbatim; documenting that here so future authors know to
        // reach for `singular_override` if they want a `Member` shape.
        assert_eq!(singularize("staff"), "staff");
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
        assert_eq!(type_stem("categories"), "Category");
        assert_eq!(type_stem("addresses"), "Address");
    }

    #[test]
    fn singular_override_wins_over_inflector() {
        let mut resource = ResourceState::new(ResourceName::new("data"));
        resource.singular_override = Some("datum".to_string());
        assert_eq!(type_stem_for_resource(&resource), "Datum");
    }

    #[test]
    fn singular_override_pascal_cases_snake_input() {
        let mut resource = ResourceState::new(ResourceName::new("staff"));
        resource.singular_override = Some("staff_member".to_string());
        assert_eq!(type_stem_for_resource(&resource), "StaffMember");
    }

    #[test]
    fn empty_singular_override_falls_back_to_inflector() {
        let mut resource = ResourceState::new(ResourceName::new("users"));
        resource.singular_override = Some(String::new());
        assert_eq!(type_stem_for_resource(&resource), "User");
    }

    #[test]
    fn no_singular_override_uses_inflector() {
        let resource = ResourceState::new(ResourceName::new("addresses"));
        assert_eq!(type_stem_for_resource(&resource), "Address");
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
    fn struct_name_for_variant_resource_honors_override() {
        let mut resource = ResourceState::new(ResourceName::new("data"));
        resource.singular_override = Some("datum".to_string());
        assert_eq!(struct_name_for_variant_resource(&resource, FieldVariant::Public), "DatumPublic");
        assert_eq!(struct_name_for_variant_resource(&resource, FieldVariant::Db), "Datum");
    }

    #[test]
    fn filter_struct_name_for_resource_honors_override() {
        let mut resource = ResourceState::new(ResourceName::new("data"));
        resource.singular_override = Some("datum".to_string());
        assert_eq!(filter_struct_name_for_resource(&resource), "DatumFilter");
    }

    #[test]
    fn sort_enum_name_for_resource_honors_override() {
        let mut resource = ResourceState::new(ResourceName::new("data"));
        resource.singular_override = Some("datum".to_string());
        assert_eq!(sort_enum_name_for_resource(&resource), "DatumSort");
    }
}
