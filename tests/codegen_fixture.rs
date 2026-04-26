use blast::state;
use blast::state::app::AppState;
use blast::state::names::{FieldName, ResourceName, SqlType};
use blast::state::resource::{
    AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, ValidatorRule, Verb, VerbState,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn write_fixture_state(project_root: &Path) {
    let state_dir = project_root.join("storage").join("blast").join("state");

    // App-wide state file is required by `marker_for_app` calls in
    // `emit_validators` (index.ts), `emit_list_query_module`, and
    // `emit_per_resource_list_helpers` (queries/index.ts).
    state::save_app(&state_dir, &AppState::new()).expect("save app.ron");

    let mut res = ResourceState::new(ResourceName::new("users"));

    let mut id_variants: BTreeSet<FieldVariant> = BTreeSet::new();
    id_variants.insert(FieldVariant::Db);
    id_variants.insert(FieldVariant::Public);
    id_variants.insert(FieldVariant::Admin);
    res.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("int8"),
            variants: id_variants,
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );

    let mut email_variants: BTreeSet<FieldVariant> = BTreeSet::new();
    email_variants.insert(FieldVariant::Db);
    email_variants.insert(FieldVariant::Insertable);
    email_variants.insert(FieldVariant::Public);
    let mut email_validators: BTreeSet<ValidatorRule> = BTreeSet::new();
    email_validators.insert(ValidatorRule::Required);
    email_validators.insert(ValidatorRule::Email);
    email_validators.insert(ValidatorRule::MaxLen(255));
    res.fields.insert(
        FieldName::new("email"),
        FieldState {
            sql_type: SqlType::new("text"),
            variants: email_variants,
            nullable: false,
            primary_key: false,
            validators: email_validators,
        },
    );

    let mut filterable: BTreeSet<FieldName> = BTreeSet::new();
    filterable.insert(FieldName::new("role"));
    filterable.insert(FieldName::new("created_at"));
    let mut sortable: BTreeSet<FieldName> = BTreeSet::new();
    sortable.insert(FieldName::new("created_at"));
    sortable.insert(FieldName::new("email"));
    res.verbs.insert(
        Verb::List,
        VerbState {
            auth: AuthMode::AuthRequired,
            list_options: Some(ListOptions {
                paginated: true,
                filterable_columns: filterable,
                sortable_columns: sortable,
                default_sort: Some(FieldName::new("-created_at")),
                max_page_size: Some(100),
            }),
        },
    );

    state::save_resource(&state_dir, &res).expect("save users.ron");
}

#[test]
fn run_frontend_emits_expected_artifacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path();
    write_fixture_state(project_root);

    blast::codegen::run_frontend(project_root).expect("codegen run");

    let validators = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("validators")
        .join("users.ts");
    let validators_src = fs::read_to_string(&validators).expect("read validators ts");
    assert!(
        validators_src.contains("validate_email"),
        "missing validate_email export: {validators_src}"
    );
    assert!(
        validators_src.contains("(v?.length ?? 0) <= 255"),
        "missing MaxLen(255) emission: {validators_src}"
    );
    assert!(
        validators_src.contains("invalid email"),
        "missing Email message: {validators_src}"
    );
    assert!(
        validators_src.contains("required"),
        "missing Required predicate: {validators_src}"
    );

    let validators_index = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("validators")
        .join("index.ts");
    let index_src = fs::read_to_string(&validators_index).expect("read validator index");
    assert!(index_src.contains("./users"));

    let list_query = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("list_query.ts");
    let list_query_src = fs::read_to_string(&list_query).expect("read list_query ts");
    assert!(list_query_src.contains("interface ListQuery"));
    assert!(list_query_src.contains("buildListQuery"));
    assert!(list_query_src.contains("parseListQuery"));

    let resource_list = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("queries")
        .join("users_list.ts");
    let resource_list_src = fs::read_to_string(&resource_list).expect("read users_list ts");
    assert!(resource_list_src.contains("'created_at'"));
    assert!(resource_list_src.contains("'email'"));
    assert!(resource_list_src.contains("'role'"));
    assert!(resource_list_src.contains("'-created_at'"));
    assert!(
        resource_list_src.contains("MAX_PAGE_SIZE: number = 100"),
        "missing per-resource max_page_size cap: {resource_list_src}"
    );
}
