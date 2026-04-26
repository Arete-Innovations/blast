
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use blast::codegen::test_scaffold::{self, Filter};
use blast::state;
use blast::state::app::AppState;
use blast::state::names::{FieldName, ResourceName, SqlType};
use blast::state::resource::{
    AuthMode, FieldState, FieldVariant, ResourceState, Verb, VerbState,
};

fn write_fixture_state(project_root: &Path) {
    let state_dir = project_root.join("storage").join("blast").join("state");

    // App-wide state file is required for the `marker_for_app` calls in
    // `test_scaffold::run` (common/mod.rs and tests/fixtures/mod.rs headers).
    state::save_app(&state_dir, &AppState::new()).expect("save app.ron");

    let mut users = ResourceState::new(ResourceName::new("users"));
    let mut id_variants = BTreeSet::new();
    id_variants.insert(FieldVariant::Db);
    id_variants.insert(FieldVariant::Public);
    users.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("int8"),
            variants: id_variants.clone(),
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );
    let mut email_variants = BTreeSet::new();
    email_variants.insert(FieldVariant::Db);
    email_variants.insert(FieldVariant::Insertable);
    email_variants.insert(FieldVariant::Public);
    users.fields.insert(
        FieldName::new("email"),
        FieldState {
            sql_type: SqlType::new("text"),
            variants: email_variants,
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
        },
    );
    for verb in [Verb::List, Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
        users.verbs.insert(
            verb,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );
    }
    state::save_resource(&state_dir, &users).expect("save users.ron");

    let mut posts = ResourceState::new(ResourceName::new("posts"));
    posts.fields.insert(
        FieldName::new("id"),
        FieldState {
            sql_type: SqlType::new("int8"),
            variants: id_variants,
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        },
    );
    posts.verbs.insert(
        Verb::Get,
        VerbState {
            auth: AuthMode::Public,
            list_options: None,
        },
    );
    state::save_resource(&state_dir, &posts).expect("save posts.ron");
}

#[test]
fn scaffold_all_emits_full_inventory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write_fixture_state(root);

    let report = test_scaffold::run(root, &Filter::All).expect("first run");

    assert!(report.skipped.is_empty(), "first run should skip nothing");
    assert!(!report.written.is_empty(), "first run should write files");

    let common = root.join("tests").join("common").join("mod.rs");
    let common_src = fs::read_to_string(&common).expect("read common mod.rs");
    assert!(common_src.contains("catalyst::testing::*"));
    assert!(common_src.contains("catalyst::fixture"));
    assert!(common_src.contains("DATABASE_URL_TEST"));

    let fixtures_mod = root.join("tests").join("fixtures").join("mod.rs");
    let fixtures_src = fs::read_to_string(&fixtures_mod).expect("read fixtures mod.rs");
    assert!(fixtures_src.contains("pub mod posts;"));
    assert!(fixtures_src.contains("pub mod users;"));

    let users_fixture = root.join("tests").join("fixtures").join("users.rs");
    let users_fixture_src = fs::read_to_string(&users_fixture).expect("read users fixture");
    assert!(users_fixture_src.contains("impl Fixture for User"));
    assert!(users_fixture_src.contains("flows::users::create::run"));

    let users_list = root
        .join("src")
        .join("flows")
        .join("generated")
        .join("users")
        .join("list.test.rs");
    let users_list_src = fs::read_to_string(&users_list).expect("read users list test");
    assert!(users_list_src.contains("run_in_test"));
    assert!(users_list_src.contains("fixture!(let _seed: User"));
    assert!(users_list_src.contains("list_baseline"));

    for verb in ["get", "create", "update", "delete"] {
        let path = root
            .join("src")
            .join("flows")
            .join("generated")
            .join("users")
            .join(format!("{}.test.rs", verb));
        assert!(path.exists(), "missing {}", path.display());
    }

    let posts_get = root
        .join("src")
        .join("flows")
        .join("generated")
        .join("posts")
        .join("get.test.rs");
    assert!(posts_get.exists(), "expected posts get scaffold");

    let users_route = root
        .join("src")
        .join("transport")
        .join("http")
        .join("generated")
        .join("users.test.rs");
    let route_src = fs::read_to_string(&users_route).expect("read users route test");
    assert!(route_src.contains("oneshot"));
    assert!(route_src.contains("StatusCode::INTERNAL_SERVER_ERROR"));
    assert!(route_src.contains("\"/api/users\""));
}

#[test]
fn scaffold_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write_fixture_state(root);

    let users_fixture = root.join("tests").join("fixtures").join("users.rs");

    let first = test_scaffold::run(root, &Filter::All).expect("first run");
    assert!(first.written.contains(&users_fixture));

    fs::write(&users_fixture, "// hand-edited body, blast must not stomp\n")
        .expect("hand-edit fixture");

    let second = test_scaffold::run(root, &Filter::All).expect("second run");
    assert!(
        second.written.iter().all(|p| p != &users_fixture),
        "second run must not rewrite hand-edited fixture"
    );
    assert!(second.skipped.contains(&users_fixture));

    let after = fs::read_to_string(&users_fixture).expect("re-read fixture");
    assert_eq!(after, "// hand-edited body, blast must not stomp\n");
}

#[test]
fn flow_filter_emits_only_matching_resource() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write_fixture_state(root);

    test_scaffold::run(root, &Filter::Flow("users".to_string())).expect("flow filter run");

    let users_list = root
        .join("src")
        .join("flows")
        .join("generated")
        .join("users")
        .join("list.test.rs");
    assert!(users_list.exists());

    let posts_get = root
        .join("src")
        .join("flows")
        .join("generated")
        .join("posts")
        .join("get.test.rs");
    assert!(!posts_get.exists(), "flow filter should skip posts");

    let users_route = root
        .join("src")
        .join("transport")
        .join("http")
        .join("generated")
        .join("users.test.rs");
    assert!(
        !users_route.exists(),
        "flow filter should NOT emit route smoke"
    );
}

#[test]
fn route_filter_emits_only_route_smoke() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write_fixture_state(root);

    test_scaffold::run(root, &Filter::Route("posts".to_string())).expect("route filter run");

    let posts_route = root
        .join("src")
        .join("transport")
        .join("http")
        .join("generated")
        .join("posts.test.rs");
    assert!(posts_route.exists());

    let posts_get = root
        .join("src")
        .join("flows")
        .join("generated")
        .join("posts")
        .join("get.test.rs");
    assert!(!posts_get.exists(), "route filter should skip flow tests");
}
