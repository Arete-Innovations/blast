//! Auth resource bundle — emits a working register/login/logout/me flow
//! into a freshly scaffolded project so `blast new myapp && cargo run`
//! gives the user `/api/auth/{register,login,logout,me}` out of the box.
//!
//! Auth is a default, not optional. There is no `--with-auth` flag and no
//! way to opt out from `blast new`. The user can edit or delete the
//! emitted custom code if they want a different scheme — fork the auth
//! flow, don't ask Blast for a knob.
//!
//! Outputs (relative to project root):
//!
//! - `storage/blast/state/resources/users.ron`
//! - `storage/blast/state/resources/sessions.ron`
//! - `src/flows/custom/auth.rs`
//! - `src/transport/http/custom/auth.rs`
//! - `src/services/custom/session_adapter.rs`
//! - `frontend/src/custom/pages/Login.vue`
//! - `frontend/src/custom/pages/Register.vue`
//! - `frontend/src/custom/stores/session.ts`
//! - `frontend/src/custom/api/client.ts`
//! - `frontend/src/custom/router/auth-guard.ts`
//!
//! Plus appends to existing `*/custom/mod.rs` files so the new modules
//! are reachable from the user app's module tree.
//!
//! File body literals live in `auth_scaffold_bodies.rs` so this module
//! stays focused on orchestration + primer construction.

use crate::error::{BlastError, BlastResult};
use crate::project::auth_scaffold_bodies as bodies;
use crate::state::names::{FieldName, ResourceName, SqlType};
use crate::state::resource::{
    AuthMode, FieldState, FieldVariant, FilterKind, ListOptions, Relation, ResourceState,
    SoftDeleteConfig, SoftDeleteDefault, ValidatorRule, Verb, VerbState,
};
use crate::state::{io as state_io, save_resource};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct EmitOutcome {
    pub written: Vec<PathBuf>,
}

/// Emit the entire auth bundle into `project_root`. Idempotent:
/// re-running on a project that already has these files will overwrite
/// generated state files (Primers) and skip `write_if_absent` custom
/// scaffold files. Custom flows/routes are emitted only when absent so
/// user edits survive.
pub fn emit(project_root: &Path) -> BlastResult<EmitOutcome> {
    let mut written: Vec<PathBuf> = Vec::new();

    let state_dir = project_root.join("storage").join("blast").join("state");
    let users_state = users_primer();
    save_resource(&state_dir, &users_state)?;
    written.push(state_io::resource_path(&state_dir, &users_state.name));

    let sessions_state = sessions_primer();
    save_resource(&state_dir, &sessions_state)?;
    written.push(state_io::resource_path(&state_dir, &sessions_state.name));

    let custom_targets: &[(&str, &str)] = &[
        ("src/flows/custom/auth.rs", bodies::AUTH_FLOW_RS),
        ("src/transport/http/custom/auth.rs", bodies::AUTH_HTTP_RS),
        (
            "src/services/custom/session_adapter.rs",
            bodies::SESSION_ADAPTER_RS,
        ),
    ];
    for (rel, body) in custom_targets {
        let target = project_root.join(rel);
        if write_if_absent(&target, body)? {
            written.push(target);
        }
    }

    let mod_targets: &[(&str, &str)] = &[
        ("src/flows/custom/mod.rs", "pub mod auth;\n"),
        ("src/transport/http/custom/mod.rs", "pub mod auth;\n"),
        ("src/services/custom/mod.rs", "pub mod session_adapter;\n"),
    ];
    for (rel, line) in mod_targets {
        let target = project_root.join(rel);
        if append_line_if_missing(&target, line)? {
            written.push(target);
        }
    }

    let fe_targets: &[(&str, &str)] = &[
        ("frontend/src/custom/pages/Login.vue", bodies::LOGIN_VUE),
        (
            "frontend/src/custom/pages/Register.vue",
            bodies::REGISTER_VUE,
        ),
        (
            "frontend/src/custom/stores/session.ts",
            bodies::SESSION_STORE_TS,
        ),
        ("frontend/src/custom/api/client.ts", bodies::API_CLIENT_TS),
        (
            "frontend/src/custom/router/auth-guard.ts",
            bodies::AUTH_GUARD_TS,
        ),
    ];
    for (rel, body) in fe_targets {
        let target = project_root.join(rel);
        if write_if_absent(&target, body)? {
            written.push(target);
        }
    }

    Ok(EmitOutcome { written })
}

fn users_primer() -> ResourceState {
    let mut state = ResourceState::new(ResourceName::new("users"));
    state.singular_override = Some("User".to_string());
    state.soft_delete = Some(SoftDeleteConfig {
        column: FieldName::new("deleted_at"),
        default_behavior: SoftDeleteDefault::ExcludeDeleted,
    });

    let all_variants: BTreeSet<FieldVariant> = [
        FieldVariant::Db,
        FieldVariant::Insertable,
        FieldVariant::Patch,
        FieldVariant::Public,
        FieldVariant::Admin,
    ]
    .into_iter()
    .collect();
    let db_only: BTreeSet<FieldVariant> = [FieldVariant::Db].into_iter().collect();
    let public_only: BTreeSet<FieldVariant> = [
        FieldVariant::Db,
        FieldVariant::Public,
        FieldVariant::Admin,
    ]
    .into_iter()
    .collect();
    let db_insertable: BTreeSet<FieldVariant> =
        [FieldVariant::Db, FieldVariant::Insertable].into_iter().collect();

    state.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("BigInt"),
            variants: public_only.clone(),
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );

    let email_validators: BTreeSet<ValidatorRule> = [
        ValidatorRule::Required,
        ValidatorRule::Email,
        ValidatorRule::MaxLen(255),
    ]
    .into_iter()
    .collect();
    state.fields.insert(
        FieldName::new("email"),
        FieldState {
            sql_type: SqlType::new("Text"),
            variants: all_variants.clone(),
            nullable: false,
            primary_key: false,
            validators: email_validators,
        },
    );

    // password_hash is db-only; never appears in Public/Admin projections.
    state.fields.insert(
        FieldName::new("password_hash"),
        FieldState {
            sql_type: SqlType::new("Text"),
            variants: db_insertable.clone(),
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );

    let role_validators: BTreeSet<ValidatorRule> = [ValidatorRule::OneOf(vec![
        "admin".to_string(),
        "user".to_string(),
    ])]
    .into_iter()
    .collect();
    state.fields.insert(
        FieldName::new("role"),
        FieldState {
            sql_type: SqlType::new("Text"),
            variants: all_variants.clone(),
            nullable: false,
            primary_key: false,
            validators: role_validators,
        },
    );

    for col in ["created_at", "updated_at"] {
        state.fields.insert(
            FieldName::new(col),
            FieldState {
                sql_type: SqlType::new("BigInt"),
                variants: public_only.clone(),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
    }

    state.fields.insert(
        FieldName::new("deleted_at"),
        FieldState {
            sql_type: SqlType::new("BigInt"),
            variants: db_only.clone(),
            nullable: true,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );

    let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
    filterable.insert(FieldName::new("email"), FilterKind::IlikeContains);
    filterable.insert(FieldName::new("role"), FilterKind::Eq);
    let sortable: BTreeSet<FieldName> = [
        FieldName::new("id"),
        FieldName::new("email"),
        FieldName::new("created_at"),
    ]
    .into_iter()
    .collect();
    let list_options = ListOptions {
        paginated: true,
        filterable_columns: filterable,
        sortable_columns: sortable,
        default_sort: Some(FieldName::new("id")),
        max_page_size: Some(100),
    };

    insert_admin_only_verbs(&mut state, Some(list_options));
    state.canonicalize();
    state
}

fn sessions_primer() -> ResourceState {
    let mut state = ResourceState::new(ResourceName::new("sessions"));
    state.singular_override = Some("Session".to_string());

    let public_variants: BTreeSet<FieldVariant> = [
        FieldVariant::Db,
        FieldVariant::Public,
        FieldVariant::Admin,
    ]
    .into_iter()
    .collect();
    let public_insertable: BTreeSet<FieldVariant> = [
        FieldVariant::Db,
        FieldVariant::Public,
        FieldVariant::Admin,
        FieldVariant::Insertable,
    ]
    .into_iter()
    .collect();
    let db_insertable: BTreeSet<FieldVariant> =
        [FieldVariant::Db, FieldVariant::Insertable].into_iter().collect();

    state.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("BigInt"),
            variants: public_variants.clone(),
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );
    state.fields.insert(
        FieldName::new("user_id"),
        FieldState {
            sql_type: SqlType::new("BigInt"),
            variants: public_insertable.clone(),
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );
    // token is db-only; treat like a secret.
    state.fields.insert(
        FieldName::new("token"),
        FieldState {
            sql_type: SqlType::new("Text"),
            variants: db_insertable.clone(),
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );
    state.fields.insert(
        FieldName::new("expires_at"),
        FieldState {
            sql_type: SqlType::new("BigInt"),
            variants: public_insertable.clone(),
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );
    state.fields.insert(
        FieldName::new("created_at"),
        FieldState {
            sql_type: SqlType::new("BigInt"),
            variants: public_variants.clone(),
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );

    let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
    filterable.insert(FieldName::new("user_id"), FilterKind::Eq);
    let sortable: BTreeSet<FieldName> = [FieldName::new("id"), FieldName::new("created_at")]
        .into_iter()
        .collect();
    let list_options = ListOptions {
        paginated: true,
        filterable_columns: filterable,
        sortable_columns: sortable,
        default_sort: Some(FieldName::new("id")),
        max_page_size: Some(100),
    };

    state.verbs.insert(
        Verb::List,
        VerbState {
            auth: AuthMode::AdminOnly,
            list_options: Some(list_options),
        },
    );
    for verb in [Verb::Get, Verb::Delete] {
        state.verbs.insert(
            verb,
            VerbState {
                auth: AuthMode::AdminOnly,
                list_options: None,
            },
        );
    }

    state.relations.insert(
        "user".to_string(),
        Relation::BelongsTo {
            table: "users".to_string(),
            fk_local_field: FieldName::new("user_id"),
        },
    );

    state.canonicalize();
    state
}

/// Helper: insert List/Get/Create/Update/Delete verbs all gated AdminOnly.
/// End-user auth flows go through bespoke /api/auth/* routes, NOT generic
/// CRUD verbs — so the CRUD surface for `users` exists only for admin.
fn insert_admin_only_verbs(state: &mut ResourceState, list_options: Option<ListOptions>) {
    state.verbs.insert(
        Verb::List,
        VerbState {
            auth: AuthMode::AdminOnly,
            list_options,
        },
    );
    for verb in [Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
        state.verbs.insert(
            verb,
            VerbState {
                auth: AuthMode::AdminOnly,
                list_options: None,
            },
        );
    }
}

fn write_if_absent(target: &Path, body: &str) -> BlastResult<bool> {
    if target.exists() {
        return Ok(false);
    }
    let parent = target
        .parent()
        .ok_or_else(|| BlastError::Invalid(format!("path has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(target, body)?;
    Ok(true)
}

/// Append `line` to `target` if the file doesn't already contain that
/// exact line. Creates the file if missing. Returns true if any write
/// happened.
fn append_line_if_missing(target: &Path, line: &str) -> BlastResult<bool> {
    let trimmed = line.trim_end_matches('\n');
    if target.exists() {
        let body = fs::read_to_string(target)?;
        if body.lines().any(|l| l.trim_end() == trimmed) {
            return Ok(false);
        }
        let needs_newline = !body.ends_with('\n');
        let mut next = body;
        if needs_newline {
            next.push('\n');
        }
        next.push_str(line);
        fs::write(target, next)?;
        Ok(true)
    } else {
        let parent = target.parent().ok_or_else(|| {
            BlastError::Invalid(format!("path has no parent: {}", target.display()))
        })?;
        fs::create_dir_all(parent)?;
        fs::write(target, line)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::io as state_io;

    fn make_project_skeleton(root: &Path) {
        for rel in [
            "src/flows/custom",
            "src/transport/http/custom",
            "src/services/custom",
            "storage/blast/state/resources",
        ] {
            fs::create_dir_all(root.join(rel)).expect("mkdir");
        }
        for rel in [
            "src/flows/custom/mod.rs",
            "src/transport/http/custom/mod.rs",
            "src/services/custom/mod.rs",
        ] {
            fs::write(root.join(rel), "// custom code lives here. blast never overwrites this directory.\n")
                .expect("seed mod.rs");
        }
    }

    #[test]
    fn emit_writes_all_expected_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        make_project_skeleton(dir.path());
        let outcome = emit(dir.path()).expect("emit");

        assert!(dir.path().join("storage/blast/state/resources/users.ron").is_file());
        assert!(dir.path().join("storage/blast/state/resources/sessions.ron").is_file());
        assert!(dir.path().join("src/flows/custom/auth.rs").is_file());
        assert!(dir.path().join("src/transport/http/custom/auth.rs").is_file());
        assert!(dir.path().join("src/services/custom/session_adapter.rs").is_file());
        assert!(dir.path().join("frontend/src/custom/pages/Login.vue").is_file());
        assert!(dir.path().join("frontend/src/custom/pages/Register.vue").is_file());
        assert!(dir.path().join("frontend/src/custom/stores/session.ts").is_file());
        assert!(dir.path().join("frontend/src/custom/api/client.ts").is_file());
        assert!(dir.path().join("frontend/src/custom/router/auth-guard.ts").is_file());

        assert!(outcome.written.len() >= 13);
    }

    #[test]
    fn emit_primers_parse_back_via_state_io() {
        let dir = tempfile::tempdir().expect("tempdir");
        make_project_skeleton(dir.path());
        emit(dir.path()).expect("emit");

        let state_dir = dir.path().join("storage/blast/state");
        let users = state_io::load_resource(&state_dir, &ResourceName::new("users"))
            .expect("load users primer");
        assert_eq!(users.name.as_str(), "users");
        assert!(users.fields.contains_key(&FieldName::new("email")));
        assert!(users.fields.contains_key(&FieldName::new("password_hash")));
        assert!(users.soft_delete.is_some());

        let sessions = state_io::load_resource(&state_dir, &ResourceName::new("sessions"))
            .expect("load sessions primer");
        assert_eq!(sessions.name.as_str(), "sessions");
        assert!(sessions.fields.contains_key(&FieldName::new("token")));
        assert!(sessions.fields.contains_key(&FieldName::new("user_id")));
        assert!(sessions.soft_delete.is_none());
    }

    #[test]
    fn emit_appends_to_custom_mod_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        make_project_skeleton(dir.path());
        emit(dir.path()).expect("emit");

        let flows_mod = fs::read_to_string(dir.path().join("src/flows/custom/mod.rs"))
            .expect("read flows mod");
        assert!(flows_mod.contains("pub mod auth;"));

        let http_mod = fs::read_to_string(dir.path().join("src/transport/http/custom/mod.rs"))
            .expect("read http mod");
        assert!(http_mod.contains("pub mod auth;"));

        let services_mod = fs::read_to_string(dir.path().join("src/services/custom/mod.rs"))
            .expect("read services mod");
        assert!(services_mod.contains("pub mod session_adapter;"));
    }

    #[test]
    fn emit_is_idempotent_for_mod_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        make_project_skeleton(dir.path());
        emit(dir.path()).expect("first emit");
        let before = fs::read_to_string(dir.path().join("src/flows/custom/mod.rs"))
            .expect("read flows mod first");
        emit(dir.path()).expect("second emit");
        let after = fs::read_to_string(dir.path().join("src/flows/custom/mod.rs"))
            .expect("read flows mod second");
        assert_eq!(before, after);
    }

    #[test]
    fn emit_does_not_overwrite_user_edited_custom_rust() {
        let dir = tempfile::tempdir().expect("tempdir");
        make_project_skeleton(dir.path());
        let custom_path = dir.path().join("src/flows/custom/auth.rs");
        fs::create_dir_all(custom_path.parent().expect("parent")).expect("mkdir");
        fs::write(&custom_path, "// user-edited\n").expect("seed user file");
        emit(dir.path()).expect("emit");
        let after = fs::read_to_string(&custom_path).expect("read");
        assert_eq!(after, "// user-edited\n");
    }

    #[test]
    fn users_primer_marks_password_hash_as_db_only() {
        let users = users_primer();
        let pw = users
            .fields
            .get(&FieldName::new("password_hash"))
            .expect("password_hash field");
        assert!(pw.variants.contains(&FieldVariant::Db));
        assert!(pw.variants.contains(&FieldVariant::Insertable));
        // Critical: never expose the hash in any public projection.
        assert!(!pw.variants.contains(&FieldVariant::Public));
        assert!(!pw.variants.contains(&FieldVariant::Admin));
    }

    #[test]
    fn sessions_primer_has_belongs_to_user_relation() {
        let sessions = sessions_primer();
        let rel = sessions.relations.get("user").expect("user relation");
        match rel {
            Relation::BelongsTo { table, fk_local_field } => {
                assert_eq!(table, "users");
                assert_eq!(fk_local_field.as_str(), "user_id");
            }
            other => panic!("expected BelongsTo, got {other:?}"),
        }
    }
}
