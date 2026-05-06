use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::{
    error::{BlastError, BlastResult},
    state::{
        gen_level::GenLevel,
        names::{FieldName, ResourceName, SqlType},
        resource::{AuthMode, CrankPolicy, FieldState, FieldVariant, ListOptions, ResourceState, SoftDeleteConfig, SoftDeleteDefault, ValidatorRule, Verb, VerbState, RESOURCE_SCHEMA_VERSION},
    },
};

use super::state::{ColumnSpec, ValidatorChoice, WizardState};

pub struct Artifacts {
    pub up_sql: String,
    pub down_sql: String,
    pub resource: ResourceState,
    pub resource_ron: String,
}

pub fn build(state: &WizardState) -> Artifacts {
    let table = state.table_name.value().trim().to_string();
    let up_sql = render_up_sql(&table, state);
    let down_sql = render_down_sql(&table);
    let resource = build_resource_state(&table, state);
    let resource_ron = preview_ron(&resource);
    Artifacts { up_sql, down_sql, resource, resource_ron }
}

/// Same as `build`, but propagates per-verb crank validation errors
/// (numeric inputs that fail to parse). Used by the renderer's preview
/// step so a malformed deadline_ms never crashes the TUI.
pub fn build_safely(state: &WizardState) -> BlastResult<Artifacts> {
    for (verb, draft) in state.per_verb_crank.iter() {
        match draft.to_policy() {
            Ok(_p) => {}
            Err(err) => return Err(BlastError::Invalid(format!("verb {:?}: {}", verb, err))),
        }
    }
    Ok(build(state))
}

fn render_up_sql(table: &str, state: &WizardState) -> String {
    let mut lines: Vec<String> = Vec::new();
    if state.id_pk {
        lines.push("    id BIGSERIAL PRIMARY KEY".to_string());
    }
    for col in &state.columns {
        lines.push(format!("    {}", render_column(col)));
    }
    if state.created_at {
        lines.push("    created_at BIGINT NOT NULL DEFAULT extract(epoch from NOW())::bigint".to_string());
    }
    if state.updated_at {
        lines.push("    updated_at BIGINT NOT NULL DEFAULT extract(epoch from NOW())::bigint".to_string());
    }
    if state.soft_delete {
        lines.push("    deleted_at BIGINT NULL".to_string());
    }

    let mut out = String::new();
    out.push_str(&format!("CREATE TABLE {} (\n", table));
    out.push_str(&lines.join(",\n"));
    out.push_str("\n);\n");
    out
}

fn render_down_sql(table: &str) -> String {
    format!("DROP TABLE IF EXISTS {};\n", table)
}

fn render_column(col: &ColumnSpec) -> String {
    let mut out = format!("{} {}", col.name, col.ty.sql_fragment());
    if col.not_null {
        out.push_str(" NOT NULL");
    }
    out
}

fn build_resource_state(table: &str, state: &WizardState) -> ResourceState {
    let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();

    if state.id_pk {
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: BTreeSet::from([FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin]),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
                kind: Default::default(),
            },
        );
    }

    for col in &state.columns {
        let mut variants = BTreeSet::from([FieldVariant::Db, FieldVariant::Admin]);
        if state.verbs.create {
            variants.insert(FieldVariant::Insertable);
        }
        if state.verbs.update {
            variants.insert(FieldVariant::Patch);
        }
        if col.public_visible {
            variants.insert(FieldVariant::Public);
        }
        let nullable = !col.not_null;
        let sql_type_label = col.ty.ron_sql_type().to_string();
        let validators = validators_for_choice(col.validator);
        fields.insert(
            FieldName::new(col.name.clone()),
            FieldState {
                sql_type: SqlType::new(sql_type_label),
                variants,
                nullable,
                primary_key: false,
                validators,
                kind: Default::default(),
            },
        );
    }

    if state.created_at {
        fields.insert(
            FieldName::new("created_at"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: BTreeSet::from([FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin]),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
                kind: Default::default(),
            },
        );
    }
    if state.updated_at {
        fields.insert(
            FieldName::new("updated_at"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: BTreeSet::from([FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin]),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
                kind: Default::default(),
            },
        );
    }
    if state.soft_delete {
        fields.insert(
            FieldName::new("deleted_at"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: BTreeSet::from([FieldVariant::Db, FieldVariant::Admin]),
                nullable: true,
                primary_key: false,
                validators: BTreeSet::new(),
                kind: Default::default(),
            },
        );
    }

    let verbs = build_verbs(state);

    let soft_delete_config = if state.soft_delete {
        Some(SoftDeleteConfig {
            column: FieldName::new("deleted_at"),
            default_behavior: SoftDeleteDefault::ExcludeDeleted,
        })
    } else {
        None
    };

    ResourceState {
        schema_version: RESOURCE_SCHEMA_VERSION,
        name: ResourceName::new(table.to_string()),
        fields,
        verbs,
        ws_events: None,
        singular_override: None,
        soft_delete: soft_delete_config,
        relations: BTreeMap::new(),
        gen_level: state.gen_level(),
        list_layout: None,
        detail_layout: None,
        toggle_endpoint: None,
        live_topics: Vec::new(),
    }
}

fn validators_for_choice(choice: ValidatorChoice) -> BTreeSet<ValidatorRule> {
    match choice {
        ValidatorChoice::None => BTreeSet::new(),
        ValidatorChoice::Required => BTreeSet::from([ValidatorRule::Required]),
        ValidatorChoice::Email => BTreeSet::from([ValidatorRule::Email]),
        ValidatorChoice::MaxLen255 => BTreeSet::from([ValidatorRule::MaxLen(255)]),
    }
}

fn build_verbs(state: &WizardState) -> IndexMap<Verb, VerbState> {
    let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
    let toggles = state.verbs;
    let resolve_auth = |v: Verb| -> AuthMode {
        match state.per_verb_auth.get(&v) {
            Some(c) => c.to_auth_mode(),
            None => AuthMode::AuthRequired,
        }
    };
    let resolve_crank = |v: Verb| -> CrankPolicy {
        match state.per_verb_crank.get(&v) {
            Some(d) => match d.to_policy() {
                Ok(p) => p,
                Err(_msg) => CrankPolicy::None,
            },
            None => CrankPolicy::None,
        }
    };
    if toggles.list {
        verbs.insert(
            Verb::List,
            VerbState {
                auth: resolve_auth(Verb::List),
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: BTreeMap::new(),
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: None,
                }),
                emit_rest_api: true,
                emit_html_page: true,
                crank_policy: resolve_crank(Verb::List),
            },
        );
    }
    if toggles.get {
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: resolve_auth(Verb::Get),
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
                crank_policy: resolve_crank(Verb::Get),
            },
        );
    }
    if toggles.create {
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: resolve_auth(Verb::Create),
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
                crank_policy: resolve_crank(Verb::Create),
            },
        );
    }
    if toggles.update {
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: resolve_auth(Verb::Update),
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
                crank_policy: resolve_crank(Verb::Update),
            },
        );
    }
    if toggles.delete {
        verbs.insert(
            Verb::Delete,
            VerbState {
                auth: resolve_auth(Verb::Delete),
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
                crank_policy: resolve_crank(Verb::Delete),
            },
        );
    }
    verbs
}

fn preview_ron(resource: &ResourceState) -> String {
    let mut canonical = resource.clone();
    canonical.canonicalize();
    let config = ron::ser::PrettyConfig::new().depth_limit(64).indentor("  ".to_string()).struct_names(true);
    match ron::ser::to_string_pretty(&canonical, config) {
        Ok(s) => format!("{}\n", s),
        Err(e) => format!("(ron preview failed: {})", e),
    }
}

/// Light-touch validation used by the per-step "next" transitions.
/// Only checks fields the user has entered up to this point; column
/// presence and verb-compat checks are deferred to `validate`.
pub fn validate_form(state: &WizardState) -> BlastResult<String> {
    let table = state.table_name.value().trim().to_string();
    if table.is_empty() {
        return Err(BlastError::Invalid("Table name is required.".to_string()));
    }
    ResourceName::try_new(table.clone())?;
    if !state.verbs.any() && state.gen_level() >= GenLevel::Route {
        return Err(BlastError::Invalid("Pick at least one verb when gen_level >= Route.".to_string()));
    }
    Ok(table)
}

/// Columns-screen → Preview-screen transition. Form-screen checks
/// PLUS column shape and verb/column compatibility.
pub fn validate(state: &WizardState) -> BlastResult<String> {
    let table = validate_form(state)?;
    if !state.id_pk && state.columns.is_empty() && !state.created_at && !state.updated_at && !state.soft_delete {
        return Err(BlastError::Invalid("Table is empty — pick at least one auto-feature or add a column.".to_string()));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for col in &state.columns {
        if col.name.trim().is_empty() {
            return Err(BlastError::Invalid("Column name cannot be empty.".to_string()));
        }
        FieldName::try_new(col.name.clone())?;
        if !seen.insert(col.name.clone()) {
            return Err(BlastError::Invalid(format!("Duplicate column name '{}'.", col.name)));
        }
        if (state.id_pk && col.name == "id") || (state.created_at && col.name == "created_at") || (state.updated_at && col.name == "updated_at") || (state.soft_delete && col.name == "deleted_at") {
            return Err(BlastError::Invalid(format!("Column '{}' collides with an auto-feature.", col.name)));
        }
    }
    if state.verbs.create && state.columns.is_empty() {
        return Err(BlastError::Invalid("Create verb needs at least one user column. Press [+ Add column] or disable Create.".to_string()));
    }
    if state.verbs.update && state.columns.is_empty() {
        return Err(BlastError::Invalid("Update verb needs at least one user column. Press [+ Add column] or disable Update.".to_string()));
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::{ColumnType, VerbToggles};
    use crate::state::names::is_snake_case_ident;

    fn empty_palette() -> Vec<ColumnType> {
        vec![ColumnType::Text]
    }

    #[test]
    fn snake_case_validator() {
        assert!(is_snake_case_ident("users"));
        assert!(is_snake_case_ident("user_roles_v2"));
        assert!(!is_snake_case_ident(""));
        assert!(!is_snake_case_ident("Users"));
        assert!(!is_snake_case_ident("1table"));
        assert!(!is_snake_case_ident("has space"));
    }

    #[test]
    fn rust_keyword_rejected_at_table_level() {
        for kw in ["type", "mod", "fn", "use", "match", "struct", "enum"] {
            let mut s = WizardState::new(std::path::PathBuf::from("."), empty_palette());
            s.table_name = tui_input::Input::new(kw.to_string());
            s.id_pk = true;
            s.verbs = VerbToggles { list: true, get: false, create: false, update: false, delete: false };
            let err = validate_form(&s).expect_err("keyword must be rejected");
            let msg = err.to_string();
            assert!(msg.contains("Rust keyword"), "expected keyword diagnostic, got: {msg}");
        }
    }

    #[test]
    fn rust_keyword_rejected_at_column_level() {
        let mut s = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        s.table_name = tui_input::Input::new("widgets".to_string());
        s.id_pk = true;
        s.verbs = VerbToggles { list: true, get: false, create: false, update: false, delete: false };
        s.columns.push(ColumnSpec { name: "type".to_string(), ty: ColumnType::Text, not_null: true, public_visible: true, validator: ValidatorChoice::None });
        let err = validate(&s).expect_err("keyword column must be rejected");
        let msg = err.to_string();
        assert!(msg.contains("Rust keyword"), "expected keyword diagnostic, got: {msg}");
    }

    #[test]
    fn renders_up_with_id_only() {
        let mut state = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        state.table_name = tui_input::Input::new("things".to_string());
        state.id_pk = true;
        state.created_at = false;
        state.updated_at = false;
        let arts = build(&state);
        assert!(arts.up_sql.contains("CREATE TABLE things ("));
        assert!(arts.up_sql.contains("id BIGSERIAL PRIMARY KEY"));
    }

    #[test]
    fn renders_up_with_user_columns_and_timestamps() {
        let mut state = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        state.table_name = tui_input::Input::new("widgets".to_string());
        state.columns.push(ColumnSpec {
            name: "name".to_string(),
            ty: ColumnType::Text,
            not_null: true,
            public_visible: true,
            validator: ValidatorChoice::None,
        });
        let arts = build(&state);
        assert!(arts.up_sql.contains("name TEXT NOT NULL"));
        assert!(arts.up_sql.contains("created_at BIGINT NOT NULL DEFAULT extract(epoch from NOW())::bigint"));
        assert!(arts.up_sql.contains("updated_at BIGINT NOT NULL DEFAULT extract(epoch from NOW())::bigint"));
    }

    #[test]
    fn renders_up_with_soft_delete() {
        let mut state = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        state.table_name = tui_input::Input::new("tombstones".to_string());
        state.soft_delete = true;
        let arts = build(&state);
        assert!(arts.up_sql.contains("deleted_at BIGINT NULL"));
        assert!(arts.resource.soft_delete.is_some());
    }

    #[test]
    fn renders_down_just_drops_table() {
        let state = {
            let mut s = WizardState::new(std::path::PathBuf::from("."), empty_palette());
            s.table_name = tui_input::Input::new("widgets".to_string());
            s
        };
        let arts = build(&state);
        assert_eq!(arts.down_sql, "DROP TABLE IF EXISTS widgets;\n");
    }

    #[test]
    fn fk_column_respects_not_null_toggle() {
        let mut state = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        state.table_name = tui_input::Input::new("posts".to_string());
        state.columns.push(ColumnSpec {
            name: "user_id".to_string(),
            ty: ColumnType::Fk("users".to_string()),
            not_null: true,
            public_visible: true,
            validator: ValidatorChoice::None,
        });
        let arts = build(&state);
        assert!(arts.up_sql.contains("user_id BIGINT REFERENCES users(id) NOT NULL"));
    }

    #[test]
    fn fk_column_nullable_when_toggled_off() {
        let mut state = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        state.table_name = tui_input::Input::new("posts".to_string());
        state.id_pk = false;
        state.created_at = false;
        state.updated_at = false;
        state.columns.push(ColumnSpec {
            name: "user_id".to_string(),
            ty: ColumnType::Fk("users".to_string()),
            not_null: false,
            public_visible: true,
            validator: ValidatorChoice::None,
        });
        let arts = build(&state);
        assert!(arts.up_sql.contains("user_id BIGINT REFERENCES users(id)"));
        assert!(!arts.up_sql.contains("NOT NULL"));
    }

    #[test]
    fn validate_rejects_empty_name() {
        let state = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        assert!(validate(&state).is_err());
    }

    #[test]
    fn validate_rejects_duplicate_columns() {
        let mut state = WizardState::new(std::path::PathBuf::from("."), empty_palette());
        state.table_name = tui_input::Input::new("widgets".to_string());
        let dup = ColumnSpec {
            name: "name".to_string(),
            ty: ColumnType::Text,
            not_null: true,
            public_visible: true,
            validator: ValidatorChoice::None,
        };
        state.columns.push(dup.clone());
        state.columns.push(dup);
        assert!(validate(&state).is_err());
    }
}
