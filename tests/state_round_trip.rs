use blast::state::{
    AppPolicySection, AppState, AuthMode, AuthScopeField, FeLintState, FieldName, FieldState,
    FieldVariant, ListOptions, PayloadShape, ResourceName, ResourceState, ServiceBackend,
    ServicesState, SqlType, TopicScope, Verb, VerbState, WsEventsState,
};
use std::collections::BTreeSet;

fn sample_resource() -> ResourceState {
    let mut res = ResourceState::new(ResourceName::new("users"));

    let mut id_variants: BTreeSet<FieldVariant> = BTreeSet::new();
    id_variants.insert(FieldVariant::Db);
    id_variants.insert(FieldVariant::Public);
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
    res.fields.insert(
        FieldName::new("email"),
        FieldState {
            sql_type: SqlType::new("text"),
            variants: email_variants,
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );

    let mut filterable: BTreeSet<FieldName> = BTreeSet::new();
    filterable.insert(FieldName::new("email"));
    res.verbs.insert(
        Verb::List,
        VerbState {
            auth: AuthMode::AuthRequired,
            list_options: Some(ListOptions {
                paginated: true,
                filterable_columns: filterable,
                sortable_columns: BTreeSet::new(),
                default_sort: None,
                max_page_size: Some(100),
            }),
        },
    );
    res.verbs.insert(
        Verb::Get,
        VerbState {
            auth: AuthMode::ScopedTo(AuthScopeField::new("owner_id")),
            list_options: None,
        },
    );

    let mut triggers: BTreeSet<FieldName> = BTreeSet::new();
    triggers.insert(FieldName::new("role"));
    res.ws_events = Some(WsEventsState {
        trigger_columns: triggers,
        payload_shape: PayloadShape::Public,
        topic_scope: TopicScope::PerRow,
    });

    res
}

fn sample_app() -> AppState {
    let mut app = AppState::new();

    let mut rules: BTreeSet<String> = BTreeSet::new();
    rules.insert("RawColorOutsidePreset".to_string());
    rules.insert("HardcodedPx".to_string());
    let fe_lint = FeLintState {
        rules,
        exempt_color_files: BTreeSet::new(),
        exempt_px_files: BTreeSet::new(),
        max_lines_per_sfc: 600,
        max_lines_per_fn: 120,
        whitelist_snippets: BTreeSet::new(),
        icon_class_patterns: BTreeSet::new(),
        scan_globs: BTreeSet::new(),
        hairline_border_rem: "0.0625rem".to_string(),
        icons_file: "src/icons.ts".to_string(),
        tokens_file: "src/styles/tokens.css".to_string(),
        primevue_preset_file: "src/plugins/primevue.ts".to_string(),
    };
    app.sections
        .insert("fe_lint".to_string(), AppPolicySection::FeLint(fe_lint));

    let services = ServicesState {
        storage: ServiceBackend::LocalDisk {
            root: "storage/uploads".to_string(),
        },
        email: ServiceBackend::Smtp {
            host: "smtp.example.com".to_string(),
            port: 587,
        },
        rate_limit: ServiceBackend::InMemory,
        session_token_ttl_seconds: 86400,
        admin_scope_fields: BTreeSet::new(),
    };
    app.sections
        .insert("services".to_string(), AppPolicySection::Services(services));

    app
}

#[test]
fn resource_round_trip_is_identity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let res = sample_resource();
    blast::state::save_resource(dir.path(), &res).expect("save");
    let loaded = blast::state::load_resource(dir.path(), &res.name).expect("load");

    let mut canonical = res.clone();
    canonical.canonicalize();
    assert_eq!(canonical, loaded);
}

#[test]
fn app_round_trip_is_identity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app = sample_app();
    blast::state::save_app(dir.path(), &app).expect("save");
    let loaded = blast::state::load_app(dir.path()).expect("load");

    let mut canonical = app.clone();
    canonical.canonicalize();
    assert_eq!(canonical, loaded);
}

#[test]
fn save_is_byte_stable() {
    let dir1 = tempfile::tempdir().expect("tempdir 1");
    let dir2 = tempfile::tempdir().expect("tempdir 2");
    let res = sample_resource();
    blast::state::save_resource(dir1.path(), &res).expect("save 1");
    blast::state::save_resource(dir2.path(), &res).expect("save 2");

    let p1 = dir1
        .path()
        .join(blast::state::io::RESOURCES_DIR)
        .join("users.ron");
    let p2 = dir2
        .path()
        .join(blast::state::io::RESOURCES_DIR)
        .join("users.ron");

    let h1 = blast::state::content_hash(&p1).expect("hash 1");
    let h2 = blast::state::content_hash(&p2).expect("hash 2");
    assert_eq!(h1, h2);
}

#[test]
fn list_resources_returns_sorted() {
    let dir = tempfile::tempdir().expect("tempdir");

    let mut zebra = ResourceState::new(ResourceName::new("zebra"));
    zebra.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("int8"),
            variants: BTreeSet::new(),
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );

    let mut alpha = ResourceState::new(ResourceName::new("alpha"));
    alpha.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("int8"),
            variants: BTreeSet::new(),
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );

    let mut middle = ResourceState::new(ResourceName::new("middle"));
    middle.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("int8"),
            variants: BTreeSet::new(),
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );

    blast::state::save_resource(dir.path(), &zebra).expect("save zebra");
    blast::state::save_resource(dir.path(), &alpha).expect("save alpha");
    blast::state::save_resource(dir.path(), &middle).expect("save middle");

    let names = blast::state::list_resources(dir.path()).expect("list");
    let raw: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    assert_eq!(raw, vec!["alpha", "middle", "zebra"]);
}

#[test]
fn list_resources_empty_dir_is_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let names = blast::state::list_resources(dir.path()).expect("list");
    assert!(names.is_empty());
}

#[test]
fn newtype_display_returns_inner() {
    let n = ResourceName::new("posts");
    assert_eq!(n.to_string(), "posts");
    assert_eq!(n.as_str(), "posts");
    let f: FieldName = "title".into();
    assert_eq!(f.to_string(), "title");
}

#[test]
fn save_writes_atomic_no_temp_left_behind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let res = sample_resource();
    blast::state::save_resource(dir.path(), &res).expect("save");
    let entries: Vec<_> = std::fs::read_dir(
        dir.path().join(blast::state::io::RESOURCES_DIR),
    )
    .expect("read_dir")
    .flatten()
    .map(|e| e.file_name().into_string().unwrap_or_default())
    .collect();
    let leftovers: Vec<&String> = entries.iter().filter(|n| n.starts_with('.')).collect();
    assert!(leftovers.is_empty(), "stray temp file: {entries:?}");
}
