use std::collections::BTreeSet;

use dialoguer::{theme::ColorfulTheme, MultiSelect};
use indexmap::IndexMap;

use crate::{
    error::BlastResult,
    schema_parser::{ParsedColumn, ParsedTable},
    state::{
        names::{FieldName, SqlType},
        resource::{FieldState, FieldVariant, ResourceState},
    },
};

const VARIANT_LABELS: &[(&str, FieldVariant)] = &[
    ("Db", FieldVariant::Db),
    ("Insertable", FieldVariant::Insertable),
    ("Patch", FieldVariant::Patch),
    ("Public", FieldVariant::Public),
    ("Admin", FieldVariant::Admin),
];

pub fn collect_fields(table: &ParsedTable, resource: &mut ResourceState) -> BlastResult<()> {
    let theme = ColorfulTheme::default();
    let mut new_fields: IndexMap<FieldName, FieldState> = IndexMap::new();

    for column in &table.columns {
        let field_name = FieldName::new(column.name.clone());
        let is_pk = table.primary_key.iter().any(|pk| pk == &column.name);
        let previous = resource.fields.get(&field_name);

        let variants = prompt_variants(&theme, column, is_pk, previous)?;
        let validators = previous_validators(previous);

        new_fields.insert(
            field_name,
            FieldState {
                sql_type: SqlType::new(column.diesel_type.clone()),
                variants,
                nullable: column.nullable,
                primary_key: is_pk,
                validators,
            },
        );
    }

    resource.fields = new_fields;
    Ok(())
}

fn previous_validators(previous: Option<&FieldState>) -> BTreeSet<crate::state::resource::ValidatorRule> {
    let Some(prev) = previous else {
        return BTreeSet::new();
    };
    prev.validators.clone()
}

fn previous_variants(previous: Option<&FieldState>, fallback: BTreeSet<FieldVariant>) -> BTreeSet<FieldVariant> {
    let Some(prev) = previous else {
        return fallback;
    };
    prev.variants.clone()
}

fn prompt_variants(theme: &ColorfulTheme, column: &ParsedColumn, is_pk: bool, previous: Option<&FieldState>) -> BlastResult<BTreeSet<FieldVariant>> {
    let defaults = previous_variants(previous, smart_defaults(&column.name, is_pk));
    let pre_selected: Vec<bool> = VARIANT_LABELS.iter().map(|(_, v)| defaults.contains(v)).collect();
    let labels: Vec<&str> = VARIANT_LABELS.iter().map(|(l, _)| *l).collect();

    let prompt = format!("Field `{}` ({}{}) — pick variants", column.name, column.diesel_type, if column.nullable { "?" } else { "" },);
    let picks = MultiSelect::with_theme(theme).with_prompt(prompt).items(&labels).defaults(&pre_selected).interact()?;

    let mut chosen: BTreeSet<FieldVariant> = BTreeSet::new();
    for idx in picks {
        let entry = VARIANT_LABELS.get(idx);
        match entry {
            Some((_, variant)) => {
                chosen.insert(*variant);
            }
            None => {}
        }
    }
    Ok(chosen)
}

pub fn smart_defaults(column_name: &str, is_pk: bool) -> BTreeSet<FieldVariant> {
    let mut set: BTreeSet<FieldVariant> = BTreeSet::new();

    if is_pk {
        set.insert(FieldVariant::Db);
        set.insert(FieldVariant::Public);
        return set;
    }

    if is_secret(column_name) {
        set.insert(FieldVariant::Db);
        return set;
    }

    if is_timestamp_audit(column_name) {
        set.insert(FieldVariant::Db);
        set.insert(FieldVariant::Public);
        return set;
    }

    set.insert(FieldVariant::Db);
    set.insert(FieldVariant::Insertable);
    set.insert(FieldVariant::Patch);
    set.insert(FieldVariant::Public);
    set.insert(FieldVariant::Admin);
    set
}

fn is_secret(column_name: &str) -> bool {
    column_name == "password_hash" || column_name.ends_with("_secret") || column_name.ends_with("_token_hash")
}

fn is_timestamp_audit(column_name: &str) -> bool {
    matches!(column_name, "created_at" | "updated_at" | "deleted_at")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pk_defaults_to_db_public() {
        let s = smart_defaults("id", true);
        assert!(s.contains(&FieldVariant::Db));
        assert!(s.contains(&FieldVariant::Public));
        assert!(!s.contains(&FieldVariant::Insertable));
    }

    #[test]
    fn password_hash_db_only() {
        let s = smart_defaults("password_hash", false);
        assert_eq!(s.len(), 1);
        assert!(s.contains(&FieldVariant::Db));
    }

    #[test]
    fn timestamp_db_public_readonly() {
        let s = smart_defaults("created_at", false);
        assert!(s.contains(&FieldVariant::Db));
        assert!(s.contains(&FieldVariant::Public));
        assert!(!s.contains(&FieldVariant::Insertable));
        assert!(!s.contains(&FieldVariant::Patch));
    }

    #[test]
    fn ordinary_column_all_variants() {
        let s = smart_defaults("email", false);
        assert_eq!(s.len(), 5);
    }
}
