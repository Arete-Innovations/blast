use crate::state::names::{FieldName, SqlType};
use crate::state::resource::ResourceState;
use std::collections::BTreeSet;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SchemaDiff {
    pub added_columns: Vec<(FieldName, SqlType)>,
    pub removed_columns: Vec<FieldName>,
    pub type_changes: Vec<(FieldName, SqlType, SqlType)>,
}

pub fn compute(schema_columns: &[(String, String)], state: &ResourceState) -> SchemaDiff {
    let schema_names: BTreeSet<&str> =
        schema_columns.iter().map(|(name, _)| name.as_str()).collect();

    let mut added_columns: Vec<(FieldName, SqlType)> = Vec::new();
    let mut type_changes: Vec<(FieldName, SqlType, SqlType)> = Vec::new();

    for (col_name, col_sql_type) in schema_columns {
        let field_name = FieldName::new(col_name.clone());
        let existing = state.fields.get(&field_name);
        match existing {
            None => {
                added_columns.push((field_name, SqlType::new(col_sql_type.clone())));
            }
            Some(field_state) => {
                if field_state.sql_type.as_str() != col_sql_type.as_str() {
                    type_changes.push((
                        field_name,
                        field_state.sql_type.clone(),
                        SqlType::new(col_sql_type.clone()),
                    ));
                }
            }
        }
    }

    let mut removed_columns: Vec<FieldName> = Vec::new();
    for state_field_name in state.fields.keys() {
        if !schema_names.contains(state_field_name.as_str()) {
            removed_columns.push(state_field_name.clone());
        }
    }

    SchemaDiff {
        added_columns,
        removed_columns,
        type_changes,
    }
}

pub fn is_empty(diff: &SchemaDiff) -> bool {
    diff.added_columns.is_empty()
        && diff.removed_columns.is_empty()
        && diff.type_changes.is_empty()
}

pub fn render(diff: &SchemaDiff) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "schema drift detected: {} added, {} removed, {} type-changed\n",
        diff.added_columns.len(),
        diff.removed_columns.len(),
        diff.type_changes.len(),
    ));

    if !diff.added_columns.is_empty() {
        out.push_str("\n  added columns (in schema.rs, not in state):\n");
        for (name, sql_type) in &diff.added_columns {
            out.push_str(&format!("    + {} : {}\n", name, sql_type));
        }
    }

    if !diff.removed_columns.is_empty() {
        out.push_str("\n  removed columns (in state, not in schema.rs):\n");
        for name in &diff.removed_columns {
            out.push_str(&format!("    - {}\n", name));
        }
    }

    if !diff.type_changes.is_empty() {
        out.push_str("\n  type changes (state vs schema.rs):\n");
        for (name, old, new) in &diff.type_changes {
            out.push_str(&format!("    ~ {} : {} -> {}\n", name, old, new));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::ResourceName;
    use crate::state::resource::{FieldState, FieldVariant};
    use indexmap::IndexMap;

    fn field(sql_type: &str) -> FieldState {
        let mut variants: BTreeSet<FieldVariant> = BTreeSet::new();
        variants.insert(FieldVariant::Db);
        FieldState {
            sql_type: SqlType::new(sql_type.to_string()),
            variants,
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        }
    }

    fn state_with(fields: &[(&str, &str)]) -> ResourceState {
        let mut state = ResourceState::new(ResourceName::new("users".to_string()));
        let mut map: IndexMap<FieldName, FieldState> = IndexMap::new();
        for (name, sql) in fields {
            map.insert(FieldName::new((*name).to_string()), field(sql));
        }
        state.fields = map;
        state
    }

    fn schema(cols: &[(&str, &str)]) -> Vec<(String, String)> {
        cols.iter()
            .map(|(n, t)| ((*n).to_string(), (*t).to_string()))
            .collect()
    }

    #[test]
    fn identical_yields_empty_diff() {
        let s = state_with(&[("id", "Integer"), ("email", "Text")]);
        let cols = schema(&[("id", "Integer"), ("email", "Text")]);
        let diff = compute(&cols, &s);
        assert!(is_empty(&diff));
    }

    #[test]
    fn added_only() {
        let s = state_with(&[("id", "Integer")]);
        let cols = schema(&[("id", "Integer"), ("email", "Text")]);
        let diff = compute(&cols, &s);
        assert_eq!(diff.added_columns.len(), 1);
        assert_eq!(diff.added_columns[0].0.as_str(), "email");
        assert_eq!(diff.added_columns[0].1.as_str(), "Text");
        assert!(diff.removed_columns.is_empty());
        assert!(diff.type_changes.is_empty());
    }

    #[test]
    fn removed_only() {
        let s = state_with(&[("id", "Integer"), ("legacy", "Text")]);
        let cols = schema(&[("id", "Integer")]);
        let diff = compute(&cols, &s);
        assert_eq!(diff.removed_columns.len(), 1);
        assert_eq!(diff.removed_columns[0].as_str(), "legacy");
        assert!(diff.added_columns.is_empty());
        assert!(diff.type_changes.is_empty());
    }

    #[test]
    fn type_changed_only() {
        let s = state_with(&[("id", "Integer"), ("count", "Integer")]);
        let cols = schema(&[("id", "Integer"), ("count", "BigInt")]);
        let diff = compute(&cols, &s);
        assert!(diff.added_columns.is_empty());
        assert!(diff.removed_columns.is_empty());
        assert_eq!(diff.type_changes.len(), 1);
        let (name, old, new) = &diff.type_changes[0];
        assert_eq!(name.as_str(), "count");
        assert_eq!(old.as_str(), "Integer");
        assert_eq!(new.as_str(), "BigInt");
    }

    #[test]
    fn mixed_changes() {
        let s = state_with(&[
            ("id", "Integer"),
            ("legacy", "Text"),
            ("count", "Integer"),
        ]);
        let cols = schema(&[
            ("id", "Integer"),
            ("count", "BigInt"),
            ("email", "Text"),
        ]);
        let diff = compute(&cols, &s);
        assert_eq!(diff.added_columns.len(), 1);
        assert_eq!(diff.removed_columns.len(), 1);
        assert_eq!(diff.type_changes.len(), 1);
        assert!(!is_empty(&diff));
    }

    #[test]
    fn render_contains_all_sections() {
        let s = state_with(&[("legacy", "Text"), ("count", "Integer")]);
        let cols = schema(&[("count", "BigInt"), ("email", "Text")]);
        let diff = compute(&cols, &s);
        let out = render(&diff);
        assert!(out.contains("added"));
        assert!(out.contains("removed"));
        assert!(out.contains("type"));
        assert!(out.contains("email"));
        assert!(out.contains("legacy"));
        assert!(out.contains("count"));
    }

    #[test]
    fn render_empty_diff_still_has_header() {
        let s = state_with(&[("id", "Integer")]);
        let cols = schema(&[("id", "Integer")]);
        let diff = compute(&cols, &s);
        let out = render(&diff);
        assert!(out.contains("0 added"));
        assert!(out.contains("0 removed"));
        assert!(out.contains("0 type-changed"));
    }
}
