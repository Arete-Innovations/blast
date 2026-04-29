use crate::{
    codegen::{
        structs::naming::type_stem_for_resource,
        validators::{render_rust::render_validators_rust_body, render_ts::render_validators_ts_body},
    },
    state::{FieldName, FieldState, FieldVariant, ResourceState, ValidatorRule},
};

pub const EMAIL_REGEX: &str = r"^[^\s@]+@[^\s@]+\.[^\s@]+$";
pub const URL_REGEX: &str = r"^https?://[^\s/$.?#].[^\s]*$";

pub fn build_resource_validators_rust(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let insertable_type = format!("{stem}Insertable");
    let patch_type = format!("{stem}Patch");

    let insertable_fields = collect_validated_fields(resource, FieldVariant::Insertable);
    let patch_fields = collect_validated_fields(resource, FieldVariant::Patch);

    render_validators_rust_body(table, &insertable_type, &patch_type, &insertable_fields, &patch_fields)
}

pub fn build_resource_validators_ts(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let insertable_type = format!("{stem}Insertable");
    let patch_type = format!("{stem}Patch");

    let insertable_fields = collect_validated_fields(resource, FieldVariant::Insertable);
    let patch_fields = collect_validated_fields(resource, FieldVariant::Patch);

    render_validators_ts_body(table, &stem, &insertable_type, &patch_type, &insertable_fields, &patch_fields)
}

pub(super) fn collect_validated_fields<'a>(resource: &'a ResourceState, variant: FieldVariant) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&variant) && !f.validators.is_empty() && !f.primary_key)
        .collect()
}

pub(super) fn any_field_uses_regex(fields: &[(&FieldName, &FieldState)]) -> bool {
    for (_, field) in fields {
        for rule in &field.validators {
            match rule {
                ValidatorRule::Email | ValidatorRule::Url | ValidatorRule::Pattern(_) => return true,
                _other => continue,
            }
        }
    }
    false
}

pub(super) fn pattern_const_name(field: &str) -> String {
    let mut out = String::with_capacity(field.len() + 11);
    for ch in field.chars() {
        for u in ch.to_uppercase() {
            out.push(u);
        }
    }
    out.push_str("_PATTERN_RE");
    out
}

pub(super) fn is_stringy(sql: &crate::state::SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "text" | "varchar" | "bpchar" | "char" | "citext" | "uuid")
}

pub(super) fn escape_rust_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

pub(super) fn escape_ts_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;

    use super::*;
    use crate::state::{
        names::{FieldName, ResourceName},
        resource::{AuthMode, FieldState, FieldVariant, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION},
        SqlType,
    };

    fn all_variants() -> BTreeSet<FieldVariant> {
        [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect()
    }

    fn id_variants() -> BTreeSet<FieldVariant> {
        [FieldVariant::Db, FieldVariant::Public].into_iter().collect()
    }

    pub(super) fn make_resource_with_validators(validators: Vec<(&str, &str, Vec<ValidatorRule>, bool)>) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: id_variants(),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        for (col, sql, rules, nullable) in validators {
            let mut rule_set = BTreeSet::new();
            for r in rules {
                rule_set.insert(r);
            }
            fields.insert(
                FieldName::new(col),
                FieldState {
                    sql_type: SqlType::new(sql),
                    variants: all_variants(),
                    nullable,
                    primary_key: false,
                    validators: rule_set,
                },
            );
        }
        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("users"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: crate::state::GenLevel::default(),
        }
    }

    #[test]
    fn email_rule_emits_byte_identical_regex_on_both_sides() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains(EMAIL_REGEX), "rust must contain literal email regex; got: {rust}");
        assert!(ts.contains(EMAIL_REGEX), "ts must contain literal email regex; got: {ts}");
    }

    #[test]
    fn url_rule_emits_byte_identical_regex_on_both_sides() {
        let r = make_resource_with_validators(vec![("homepage", "Varchar", vec![ValidatorRule::Url], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains(URL_REGEX), "rust must contain literal url regex; got: {rust}");
        assert!(ts.contains(URL_REGEX), "ts must contain literal url regex; got: {ts}");
    }

    #[test]
    fn pattern_rule_emits_user_pattern_verbatim_on_both_sides() {
        let pat = "^[a-z]+$".to_string();
        let r = make_resource_with_validators(vec![("slug", "Varchar", vec![ValidatorRule::Pattern(pat.clone())], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains(&pat));
        assert!(ts.contains(&pat));
    }

    #[test]
    fn required_rule_emits_is_empty_check() {
        let r = make_resource_with_validators(vec![("title", "Varchar", vec![ValidatorRule::Required], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains("is_empty"), "rust must use is_empty; got: {rust}");
        assert!(ts.contains("length === 0"), "ts must use length === 0; got: {ts}");
    }

    #[test]
    fn min_len_rule_emits_count_check() {
        let r = make_resource_with_validators(vec![("title", "Varchar", vec![ValidatorRule::MinLen(3)], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains("chars().count() < 3"), "rust min_len; got: {rust}");
        assert!(ts.contains("].length < 3"), "ts min_len; got: {ts}");
    }

    #[test]
    fn max_len_rule_emits_count_check() {
        let r = make_resource_with_validators(vec![("title", "Varchar", vec![ValidatorRule::MaxLen(200)], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains("chars().count() > 200"), "rust max_len; got: {rust}");
        assert!(ts.contains("].length > 200"), "ts max_len; got: {ts}");
    }

    #[test]
    fn min_value_rule_emits_numeric_check() {
        let r = make_resource_with_validators(vec![("age", "Int4", vec![ValidatorRule::MinValue(0)], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains(") < 0"), "rust min_value; got: {rust}");
        assert!(ts.contains("< 0"), "ts min_value; got: {ts}");
    }

    #[test]
    fn max_value_rule_emits_numeric_check() {
        let r = make_resource_with_validators(vec![("age", "Int4", vec![ValidatorRule::MaxValue(150)], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains(") > 150"), "rust max_value; got: {rust}");
        assert!(ts.contains("> 150"), "ts max_value; got: {ts}");
    }

    #[test]
    fn one_of_rule_emits_array_membership_check() {
        let r = make_resource_with_validators(vec![("role", "Varchar", vec![ValidatorRule::OneOf(vec!["a".to_string(), "b".to_string()])], false)]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains("\"a\""), "rust one_of array element; got: {rust}");
        assert!(rust.contains("\"b\""));
        assert!(ts.contains("'a'"), "ts one_of array element; got: {ts}");
        assert!(ts.contains("'b'"));
    }

    #[test]
    fn skips_field_with_no_validators() {
        let mut r = make_resource_with_validators(vec![]);
        r.fields.insert(
            FieldName::new("description"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_variants(),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(!rust.contains("\"description\""));
        assert!(!ts.contains("input.description"));
    }

    #[test]
    fn rust_validator_signature_matches_spec() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let rust = build_resource_validators_rust(&r);
        assert!(rust.contains("pub fn validate_users_insertable(input: &UserInsertable) -> ::std::result::Result<(), MeltDown>"), "rust signature wrong; got: {rust}");
        assert!(rust.contains("pub fn validate_users_patch(input: &UserPatch) -> ::std::result::Result<(), MeltDown>"), "rust patch signature wrong; got: {rust}");
    }

    #[test]
    fn ts_validator_signature_matches_spec() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let ts = build_resource_validators_ts(&r);
        assert!(ts.contains("export function validateUserInsertable(input: UserInsertable): FieldErrors | null"), "ts signature wrong; got: {ts}");
        assert!(ts.contains("export function validateUserPatch(input: UserPatch): FieldErrors | null"), "ts patch signature wrong; got: {ts}");
    }

    #[test]
    fn primary_key_field_skipped_even_if_validators_set() {
        let mut r = make_resource_with_validators(vec![]);
        let mut rules: BTreeSet<ValidatorRule> = BTreeSet::new();
        rules.insert(ValidatorRule::MinValue(1));
        r.fields.get_mut(&FieldName::new("id")).expect("id field present").validators = rules;
        let rust = build_resource_validators_rust(&r);
        assert!(!rust.contains("input.id"), "must skip primary key; got: {rust}");
    }

    #[test]
    fn rust_imports_meltdown_and_dto_types() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let rust = build_resource_validators_rust(&r);
        assert!(rust.contains("use crate::meltdown::MeltDown;"), "must import MeltDown");
        assert!(rust.contains("use crate::structs::generated::users::{UserInsertable, UserPatch};"), "must import DTO types");
    }

    #[test]
    fn ts_imports_dto_types() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let ts = build_resource_validators_ts(&r);
        assert!(ts.contains("import type { UserInsertable, UserPatch } from '@/generated/types/users'"), "must import DTO types; got: {ts}");
    }

    #[test]
    fn rust_uses_validation_failed_field_constructor() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let rust = build_resource_validators_rust(&r);
        assert!(rust.contains("MeltDown::validation_failed_field("), "rust must call validation_failed_field; got: {rust}");
    }

    #[test]
    fn patch_validator_wraps_in_optional_check() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let rust = build_resource_validators_rust(&r);
        assert!(rust.contains("input.email.as_ref()"), "patch wraps in Option check; got: {rust}");
        let ts = build_resource_validators_ts(&r);
        assert!(ts.contains("input.email !== undefined"), "ts patch checks defined; got: {ts}");
    }

    #[test]
    fn nullable_insertable_field_wraps_in_option_check() {
        let r = make_resource_with_validators(vec![("nickname", "Varchar", vec![ValidatorRule::MinLen(2)], true)]);
        let rust = build_resource_validators_rust(&r);
        assert!(rust.contains("input.nickname.as_ref()"), "nullable field wraps in Option; got: {rust}");
    }

    #[test]
    fn empty_validators_function_emits_compiles() {
        let r = make_resource_with_validators(vec![]);
        let rust = build_resource_validators_rust(&r);
        let ts = build_resource_validators_ts(&r);
        assert!(rust.contains("validate_users_insertable"));
        assert!(rust.contains("Ok(())"));
        assert!(ts.contains("validateUserInsertable"));
    }

    #[test]
    fn no_diesel_prelude_import() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let rust = build_resource_validators_rust(&r);
        assert!(!rust.contains("diesel::prelude"), "must not import diesel::prelude");
    }

    #[test]
    fn uses_qualified_std_result_to_pass_type_lint() {
        let r = make_resource_with_validators(vec![("email", "Varchar", vec![ValidatorRule::Email], false)]);
        let rust = build_resource_validators_rust(&r);
        assert!(rust.contains("::std::result::Result<(), MeltDown>"), "must use qualified ::std::result::Result; got: {rust}");
    }
}
