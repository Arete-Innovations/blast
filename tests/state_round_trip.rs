use blast::state::app::{
    AppState, DefaultsState, ServiceBackend, ServicesState,
};
use blast::state::names::{AuthScopeField, FieldName, ResourceName, SqlType};
use blast::state::resource::{
    AuthMode, FieldState, FieldVariant, FilterKind, ListOptions, PayloadShape, Relation,
    ResourceState, SoftDeleteConfig, SoftDeleteDefault, TopicScope, Verb, VerbState,
    WsEventsState,
};
use blast::state::{AppPolicySection, FeLintState};
use std::collections::{BTreeMap, BTreeSet};

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

    let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
    filterable.insert(FieldName::new("email"), FilterKind::Eq);
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
        max_template_depth: 5,
        max_template_loc: 200,
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
    blast::state::io::save_resource(dir.path(), &res).expect("save");
    let loaded = blast::state::io::load_resource(dir.path(), &res.name).expect("load");

    let mut canonical = res.clone();
    canonical.canonicalize();
    assert_eq!(canonical, loaded);
}

#[test]
fn app_round_trip_is_identity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app = sample_app();
    blast::state::io::save_app(dir.path(), &app).expect("save");
    let loaded = blast::state::io::load_app(dir.path()).expect("load");

    let mut canonical = app.clone();
    canonical.canonicalize();
    assert_eq!(canonical, loaded);
}

#[test]
fn save_is_byte_stable() {
    let dir1 = tempfile::tempdir().expect("tempdir 1");
    let dir2 = tempfile::tempdir().expect("tempdir 2");
    let res = sample_resource();
    blast::state::io::save_resource(dir1.path(), &res).expect("save 1");
    blast::state::io::save_resource(dir2.path(), &res).expect("save 2");

    let p1 = dir1
        .path()
        .join(blast::state::io::RESOURCES_DIR)
        .join("users.ron");
    let p2 = dir2
        .path()
        .join(blast::state::io::RESOURCES_DIR)
        .join("users.ron");

    let h1 = blast::state::hash::content_hash(&p1).expect("hash 1");
    let h2 = blast::state::hash::content_hash(&p2).expect("hash 2");
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

    blast::state::io::save_resource(dir.path(), &zebra).expect("save zebra");
    blast::state::io::save_resource(dir.path(), &alpha).expect("save alpha");
    blast::state::io::save_resource(dir.path(), &middle).expect("save middle");

    let names = blast::state::io::list_resources(dir.path()).expect("list");
    let raw: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    assert_eq!(raw, vec!["alpha", "middle", "zebra"]);
}

#[test]
fn list_resources_empty_dir_is_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let names = blast::state::io::list_resources(dir.path()).expect("list");
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
    blast::state::io::save_resource(dir.path(), &res).expect("save");
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

// ----- v2 schema-extension coverage -----

/// Build a resource that exercises every v2-only field:
/// - typed `filterable_columns` with mixed `FilterKind` operators
/// - `singular_override`
/// - `soft_delete` config
/// - `relations` (one BelongsTo, one HasMany)
fn fully_loaded_v2_resource() -> ResourceState {
    let mut res = ResourceState::new(ResourceName::new("posts"));
    res.singular_override = Some("Post".to_string());
    res.soft_delete = Some(SoftDeleteConfig {
        column: FieldName::new("deleted_at"),
        default_behavior: SoftDeleteDefault::ExcludeDeleted,
    });

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
    let mut title_variants: BTreeSet<FieldVariant> = BTreeSet::new();
    title_variants.insert(FieldVariant::Db);
    title_variants.insert(FieldVariant::Insertable);
    title_variants.insert(FieldVariant::Public);
    res.fields.insert(
        FieldName::new("title"),
        FieldState {
            sql_type: SqlType::new("text"),
            variants: title_variants,
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );

    let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
    filterable.insert(FieldName::new("title"), FilterKind::IlikeContains);
    filterable.insert(FieldName::new("created_at"), FilterKind::Range);
    filterable.insert(FieldName::new("published"), FilterKind::Bool);
    res.verbs.insert(
        Verb::List,
        VerbState {
            auth: AuthMode::Public,
            list_options: Some(ListOptions {
                paginated: true,
                filterable_columns: filterable,
                sortable_columns: BTreeSet::new(),
                default_sort: None,
                max_page_size: Some(50),
            }),
        },
    );

    res.relations.insert(
        "author".to_string(),
        Relation::BelongsTo {
            table: "users".to_string(),
            fk_local_field: FieldName::new("author_id"),
        },
    );
    res.relations.insert(
        "comments".to_string(),
        Relation::HasMany {
            table: "comments".to_string(),
            fk_remote_field: FieldName::new("post_id"),
        },
    );

    res
}

#[test]
fn v2_resource_round_trips_with_all_extensions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let res = fully_loaded_v2_resource();
    blast::state::io::save_resource(dir.path(), &res).expect("save v2");
    let loaded = blast::state::io::load_resource(dir.path(), &res.name).expect("load v2");

    let mut canonical = res.clone();
    canonical.canonicalize();
    assert_eq!(canonical, loaded, "v2 round-trip preserves all extension fields");

    // Spot-check the typed FilterKind survived.
    let list_state = loaded.verbs.get(&Verb::List).expect("list verb");
    let opts = list_state.list_options.as_ref().expect("list_options");
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("title")),
        Some(&FilterKind::IlikeContains),
        "title FilterKind survived round-trip",
    );
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("created_at")),
        Some(&FilterKind::Range),
    );
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("published")),
        Some(&FilterKind::Bool),
    );
}

#[test]
fn v2_resource_save_is_byte_stable() {
    let dir1 = tempfile::tempdir().expect("tempdir 1");
    let dir2 = tempfile::tempdir().expect("tempdir 2");
    let res = fully_loaded_v2_resource();
    blast::state::io::save_resource(dir1.path(), &res).expect("save 1");
    blast::state::io::save_resource(dir2.path(), &res).expect("save 2");

    let p1 = dir1
        .path()
        .join(blast::state::io::RESOURCES_DIR)
        .join("posts.ron");
    let p2 = dir2
        .path()
        .join(blast::state::io::RESOURCES_DIR)
        .join("posts.ron");
    let h1 = blast::state::hash::content_hash(&p1).expect("hash 1");
    let h2 = blast::state::hash::content_hash(&p2).expect("hash 2");
    assert_eq!(h1, h2, "v2 serialization is deterministic");
}

#[test]
fn v1_ron_file_loads_via_upgrader_to_v2() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resources_dir = dir.path().join(blast::state::io::RESOURCES_DIR);
    std::fs::create_dir_all(&resources_dir).expect("mk resources dir");

    // A hand-rolled v1 RON file: filterable_columns is a SEQUENCE
    // (BTreeSet shape). The new fields (singular_override, soft_delete,
    // relations) are absent — the upgrader fills them via serde defaults.
    let v1_body = r#"(
    schema_version: 1,
    name: "users",
    fields: {
        "email": (
            sql_type: "text",
            variants: [Db, Insertable, Public],
            nullable: false,
            primary_key: false,
            validators: [],
        ),
        "id": (
            sql_type: "int8",
            variants: [Db, Public],
            nullable: false,
            primary_key: true,
            validators: [],
        ),
    },
    verbs: {
        List: (
            auth: AuthRequired,
            list_options: Some((
                paginated: true,
                filterable_columns: ["email", "id"],
                sortable_columns: [],
                default_sort: None,
                max_page_size: Some(100),
            )),
        ),
    },
    ws_events: None,
)
"#;
    let v1_path = resources_dir.join("users.ron");
    std::fs::write(&v1_path, v1_body).expect("write v1 file");

    let loaded = blast::state::io::load_resource(dir.path(), &ResourceName::new("users"))
        .expect("upgrade-then-load v1 file");

    assert_eq!(
        loaded.schema_version, 2,
        "schema_version should be bumped to 2 after upgrade"
    );
    assert_eq!(loaded.singular_override, None, "default singular_override absent");
    assert_eq!(loaded.soft_delete, None, "default soft_delete absent");
    assert!(loaded.relations.is_empty(), "default relations empty");

    let list_state = loaded.verbs.get(&Verb::List).expect("List verb present");
    let opts = list_state
        .list_options
        .as_ref()
        .expect("list_options present");
    assert_eq!(opts.filterable_columns.len(), 2, "two filterable cols");
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("email")),
        Some(&FilterKind::Eq),
        "v1 columns default to FilterKind::Eq",
    );
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("id")),
        Some(&FilterKind::Eq),
    );
}

/// Pin the canonical default `users.ron` Primer that ships with every
/// scaffolded project. Asserts the file at
/// `templates/canonical/storage/blast/state/resources/users.ron` parses
/// cleanly and matches the in-code spec we expect codegen to consume on
/// first scaffold.
#[test]
fn canonical_template_users_ron_loads_cleanly() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let state_dir = manifest
        .join("templates")
        .join("canonical")
        .join("storage")
        .join("blast")
        .join("state");

    let loaded = blast::state::io::load_resource(
        &state_dir,
        &ResourceName::new("users"),
    )
    .expect("load templates canonical users.ron");

    assert_eq!(loaded.name.as_str(), "users");
    assert_eq!(loaded.singular_override.as_deref(), Some("User"));

    // Soft-delete configured on the deleted_at column.
    let sd = loaded.soft_delete.as_ref().expect("soft_delete configured");
    assert_eq!(sd.column.as_str(), "deleted_at");
    assert_eq!(sd.default_behavior, SoftDeleteDefault::ExcludeDeleted);

    // Every CRUD verb is admin-only on the canonical resource.
    for v in [Verb::List, Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
        let state = loaded.verbs.get(&v).unwrap_or_else(|| panic!("verb {v:?} present"));
        assert!(
            matches!(state.auth, AuthMode::AdminOnly),
            "verb {v:?} should be AdminOnly, got {:?}",
            state.auth
        );
    }

    // password_hash MUST NEVER be in the Public projection.
    let pwd = loaded
        .fields
        .get(&FieldName::new("password_hash"))
        .expect("password_hash field");
    assert!(
        !pwd.variants.contains(&FieldVariant::Public),
        "password_hash leaked into Public projection: {:?}",
        pwd.variants
    );
    assert!(
        pwd.variants.contains(&FieldVariant::Admin),
        "password_hash missing from Admin projection: {:?}",
        pwd.variants
    );

    // List verb has paginated + filterable + sortable + max_page_size.
    let list = loaded.verbs.get(&Verb::List).expect("list verb");
    let opts = list.list_options.as_ref().expect("list_options");
    assert!(opts.paginated, "list paginated");
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("email")),
        Some(&FilterKind::IlikeContains),
    );
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("role")),
        Some(&FilterKind::Eq),
    );
    assert_eq!(
        opts.filterable_columns.get(&FieldName::new("created_at")),
        Some(&FilterKind::Range),
    );
    assert!(opts.sortable_columns.contains(&FieldName::new("id")));
    assert!(opts.sortable_columns.contains(&FieldName::new("email")));
    assert!(opts.sortable_columns.contains(&FieldName::new("created_at")));
}

#[test]
fn defaults_section_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = AppState::new();
    let defaults = DefaultsState {
        soft_delete_new_resources_default: Some(SoftDeleteDefault::ExcludeDeleted),
    };
    app.sections
        .insert("defaults".to_string(), AppPolicySection::Defaults(defaults));

    blast::state::io::save_app(dir.path(), &app).expect("save app with defaults");
    let loaded = blast::state::io::load_app(dir.path()).expect("load app with defaults");

    let mut canonical = app.clone();
    canonical.canonicalize();
    assert_eq!(canonical, loaded);

    let section = loaded
        .sections
        .get("defaults")
        .expect("defaults section present");
    match section {
        AppPolicySection::Defaults(state) => {
            assert_eq!(
                state.soft_delete_new_resources_default,
                Some(SoftDeleteDefault::ExcludeDeleted),
            );
        }
        other => panic!("expected Defaults variant, got {other:?}"),
    }
}
