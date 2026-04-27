//! End-to-end smoke test for the BE/FE enum codegen seam.
//!
//! Drives the in-process codegen passes against a temp project to prove
//! that a `CREATE TYPE` migration + a resource referencing the PascalCased
//! Diesel type name flows through:
//!
//!   - `codegen::enums::run` (Rust per-enum file + barrel)
//!   - `codegen::frontend_types::run` (TS string-literal-union + values const)
//!   - `codegen::components::run` (Vue Dropdown wiring with :options binding)
//!
//! The test stubs `src/database/schema.rs` by hand because the real
//! schema_gen pass requires a live Postgres + diesel print-schema, which is
//! out of scope for a focused codegen smoke.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use indexmap::IndexMap;

use blast::codegen;
use blast::io::{NullProgress, NullSink};
use blast::state::{
    save_app, save_resource, AppState, AuthMode, FieldName, FieldState, FieldVariant, GenLevel,
    ResourceName, ResourceState, SqlType, Verb, VerbState,
};
use blast::state::resource::RESOURCE_SCHEMA_VERSION;

fn write_file(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir parent");
    }
    fs::write(path, body).expect("write file");
}

fn seed_migration(root: &Path, dir: &str, body: &str) {
    let migration_dir = root.join("src/database/migrations").join(dir);
    fs::create_dir_all(&migration_dir).expect("mkdir migration");
    fs::write(migration_dir.join("up.sql"), body).expect("write up.sql");
}

fn seed_schema_stub(root: &Path) {
    let schema_path = root.join("src/database/schema.rs");
    let body = "pub mod sql_types {\n    #[derive(diesel::sql_types::SqlType)]\n    #[diesel(postgres_type(name = \"task_status\"))]\n    pub struct TaskStatus;\n}\n";
    write_file(&schema_path, body);
}

fn seed_app_state(root: &Path) {
    let state_dir = root.join("storage/blast/state");
    save_app(&state_dir, &AppState::new()).expect("save app");
}

fn seed_resource(root: &Path) {
    let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();

    let id_variants: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public].into_iter().collect();
    fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("Int8"),
            variants: id_variants,
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );

    let title_variants: BTreeSet<FieldVariant> = [
        FieldVariant::Db,
        FieldVariant::Insertable,
        FieldVariant::Patch,
        FieldVariant::Public,
    ]
    .into_iter()
    .collect();
    fields.insert(
        FieldName::new("title"),
        FieldState {
            sql_type: SqlType::new("Varchar"),
            variants: title_variants,
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );

    let status_variants: BTreeSet<FieldVariant> = [
        FieldVariant::Db,
        FieldVariant::Insertable,
        FieldVariant::Patch,
        FieldVariant::Public,
    ]
    .into_iter()
    .collect();
    fields.insert(
        FieldName::new("status"),
        FieldState {
            sql_type: SqlType::new("TaskStatus"),
            variants: status_variants,
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );

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
    verbs.insert(
        Verb::Get,
        VerbState {
            auth: AuthMode::Public,
            list_options: None,
        },
    );

    let resource = ResourceState {
        schema_version: RESOURCE_SCHEMA_VERSION,
        name: ResourceName::new("tasks"),
        fields,
        verbs,
        ws_events: None,
        singular_override: None,
        soft_delete: None,
        relations: BTreeMap::new(),
        gen_level: GenLevel::Components,
    };

    let state_dir = root.join("storage/blast/state");
    save_resource(&state_dir, &resource).expect("save resource");
}

#[test]
fn enum_codegen_chain_threads_be_to_fe_for_dropdown_field() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    seed_migration(
        root,
        "2026-04-26-000001_tasks",
        "CREATE TYPE task_status AS ENUM ('pending', 'active', 'done');\n",
    );
    seed_schema_stub(root);
    seed_app_state(root);
    seed_resource(root);

    let mut sink = NullSink;
    let mut progress = NullProgress;

    codegen::enums::run(root, &mut sink, &mut progress).expect("enums runner ok");
    codegen::frontend_types::run(root, &mut sink, &mut progress).expect("frontend_types runner ok");
    codegen::components::run(root, &mut sink, &mut progress).expect("components runner ok");

    let rust_enum = root.join("src/structs/generated/enums/task_status.rs");
    assert!(rust_enum.exists(), "task_status.rs must exist");
    let rust_body = fs::read_to_string(&rust_enum).expect("read task_status.rs");
    assert!(rust_body.contains("pub enum TaskStatus {"), "TaskStatus enum decl missing: {rust_body}");
    assert!(rust_body.contains("    Pending,"), "Pending variant missing");
    assert!(rust_body.contains("    Active,"), "Active variant missing");
    assert!(rust_body.contains("    Done,"), "Done variant missing");
    assert!(rust_body.contains("TaskStatus::Pending => \"pending\""), "as_str pending arm missing");
    assert!(rust_body.contains("TaskStatus::Active => \"active\""), "as_str active arm missing");
    assert!(rust_body.contains("TaskStatus::Done => \"done\""), "as_str done arm missing");

    let ts_enum = root.join("frontend/src/generated/types/task_status.ts");
    assert!(ts_enum.exists(), "task_status.ts must exist");
    let ts_body = fs::read_to_string(&ts_enum).expect("read task_status.ts");
    assert!(
        ts_body.contains("export type TaskStatus = 'pending' | 'active' | 'done'"),
        "TS string-literal union missing: {ts_body}"
    );
    assert!(
        ts_body.contains("export const TASK_STATUS_VALUES: readonly TaskStatus[] = ['pending', 'active', 'done'] as const"),
        "TS values const missing: {ts_body}"
    );

    let ts_resource = root.join("frontend/src/generated/types/tasks.ts");
    assert!(ts_resource.exists(), "tasks.ts must exist");
    let ts_resource_body = fs::read_to_string(&ts_resource).expect("read tasks.ts");
    assert!(
        ts_resource_body.contains("import type { TaskStatus } from './task_status'"),
        "tasks.ts must import TaskStatus alias: {ts_resource_body}"
    );
    assert!(
        ts_resource_body.contains("  status: TaskStatus"),
        "status field must use TaskStatus alias: {ts_resource_body}"
    );

    let create_form = root.join("frontend/src/components/generated/forms/tasks/CreateForm.vue");
    assert!(create_form.exists(), "CreateForm.vue must exist");
    let create_body = fs::read_to_string(&create_form).expect("read CreateForm.vue");
    assert!(
        create_body.contains("from 'primevue/dropdown'"),
        "Dropdown import missing: {create_body}"
    );
    assert!(
        create_body.contains("import { TASK_STATUS_VALUES } from '@/generated/types/task_status'"),
        "TASK_STATUS_VALUES import missing: {create_body}"
    );
    assert!(
        create_body.contains(":options=\"TASK_STATUS_VALUES\""),
        "Dropdown options binding missing: {create_body}"
    );

    let edit_form = root.join("frontend/src/components/generated/forms/tasks/EditForm.vue");
    assert!(edit_form.exists(), "EditForm.vue must exist");
    let edit_body = fs::read_to_string(&edit_form).expect("read EditForm.vue");
    assert!(
        edit_body.contains("from 'primevue/dropdown'"),
        "Dropdown import missing in EditForm: {edit_body}"
    );
    assert!(
        edit_body.contains(":options=\"TASK_STATUS_VALUES\""),
        "Dropdown options binding missing in EditForm: {edit_body}"
    );
}
