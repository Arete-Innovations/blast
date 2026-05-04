//! End-to-end smoke test for the full blast lifecycle.
//!
//! This test drives the actual `blast` binary through the full project
//! lifecycle to prove that the generator + codegen pipeline + generated
//! Rust code holds together as a system. Concretely it:
//!
//! 1. Creates a fresh temp directory.
//! 2. Runs `blast new test_app --dev` to scaffold a Catablast app.
//! 3. Patches the scaffolded `Cargo.toml` to depend on the local catalyst checkout (instead of crates.io / git) so we test against the same catalyst we develop against.
//! 4. Manually writes a minimal `users` resource to `storage/blast/state/resources/users.ron` using the typed `blast::state::save_resource` API. (Doing this via the TUI wizard would require driving stdin keystrokes —
//!    too brittle for a smoke test.)
//! 5. Runs `blast gen all` to drive the full codegen pipeline (schema → structs → models → flows → http → frontend → ws → vue → env-example → governor-plugin → test-scaffolds).
//! 6. Runs `cargo build` inside the generated app to prove the emitted Rust compiles against catalyst.
//!
//! ## Status (2026-04-26)
//!
//! Marked `#[ignore]` because it WILL FAIL today: catalyst is missing
//! Wave-4 primitives that Wave-3 codegen references — `Ctx::require_admin`,
//! `Ctx::require_roles`, the `transport::http::list_query` module
//! (`ListQuery` + `ListResponse`), and the per-FieldVariant projection
//! structs (`<Resource>InsertableForCreate`, `<Resource>PatchForUpdate`,
//! `<Resource>PublicRow`, `<Resource>AdminRow`, `<Resource>Filter`) which
//! the structs codegen v2 still has to emit. Once those land, this should
//! flip green and we can drop the `#[ignore]`.
//!
//! The harness *itself* compiles and is structurally complete — the
//! purpose of landing it now is to lock in the surface so Wave-4 has a
//! clear acceptance target.
//!
//! ## How to run
//!
//! ```bash
//! cargo test --test e2e_smoke -- --ignored
//! ```
//!
//! ## Open TODOs to actually pass
//!
//! - **catalyst**: ship `Ctx::require_admin`, `Ctx::require_roles`, `catalyst::transport::http::list_query` module, and the catalyst-side testing harness used by scaffolded test files.
//! - **blast**: structs.rs codegen v2 (per-variant projections).
//! - **blast new --dev template**: switch the cloned template from the legacy Rocket-based catalyst (`catalyst/dev`) to a thin scaffold that depends on the new axum catalyst as a Cargo dependency. Until then the patch
//!   step below is a stopgap: it inserts a `[patch.crates-io]` stanza pointing catalyst at the local checkout, but the cloned template still IS catalyst so the patch is a no-op until the template is rewritten.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use blast::state::{
    self,
    names::{FieldName, ResourceName, SqlType},
    resource::{AuthMode, FieldState, FieldVariant, FilterKind, ListOptions, ResourceState, Verb, VerbState},
};
use indexmap::IndexMap;

/// Replace the dbname segment of a Postgres URL. Used by the e2e harness
/// to scrub the user-supplied admin URL into a per-test sentinel before
/// `blast new` creates databases.
fn swap_dbname(template: &str, new_dbname: &str) -> String {
    let parsed = blast::project::db_bootstrap::parse_url(template).expect("BLAST_TEST_DB_URL must be a valid Postgres URL");
    parsed.with_dbname(new_dbname).rebuild()
}

/// Path to the catalyst checkout we want the scaffolded app to depend on.
/// Resolved at compile time from `CARGO_MANIFEST_DIR` (the blast crate
/// root) by walking up to `catablast/` and into `catalyst/`.
fn local_catalyst_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR is the blast crate root, which may live at:
    //   - <catablast>/blast               (master)
    //   - <catablast>/blast/worktrees/<name>  (worktree)
    // Both cases need to climb back to <catablast> and into catalyst.
    let mut probe = manifest_dir.clone();
    loop {
        let candidate = probe.join("catalyst");
        if candidate.is_dir() {
            return candidate;
        }
        match probe.parent() {
            Some(parent) => probe = parent.to_path_buf(),
            None => panic!("could not find catalyst checkout above blast manifest dir {}", manifest_dir.display()),
        }
    }
}

#[test]
#[ignore]
fn full_blast_lifecycle_smokes() {
    let blast_bin: &str = env!("CARGO_BIN_EXE_blast");
    let catalyst_path = local_catalyst_path();

    // 1. Fresh tempdir as the workspace.
    let tmp = tempfile::tempdir().expect("create tempdir");
    let workspace = tmp.path();
    let project_name = "test_app";
    let project_dir = workspace.join(project_name);

    // 2. blast new <name> --dev (scaffold).
    //
    // `blast new` now hard-fails if it can't reach Postgres, so this test
    // requires `BLAST_TEST_DB_URL` to point at a reachable admin-capable
    // Postgres instance. The dbname segment is replaced at runtime; pick a
    // throwaway URL like `postgres://postgres:postgres@localhost:5432/postgres`.
    let db_template = match std::env::var("BLAST_TEST_DB_URL") {
        Ok(v) => v,
        Err(_e) => {
            eprintln!("skipping e2e_smoke: BLAST_TEST_DB_URL not set");
            return;
        }
    };
    let db_url = swap_dbname(&db_template, "test_app_e2e");
    let new_out = run_blast(blast_bin, workspace, &["new", project_name, "--dev", "--db-url", &db_url, "--force"]);
    assert_step_succeeded("blast new", &new_out);
    assert!(project_dir.is_dir(), "blast new did not create project dir at {}", project_dir.display());

    // 3. Patch Cargo.toml so the scaffolded app uses the local catalyst checkout instead of whatever crates.io / git source the template declared. Inserts a `[patch.crates-io]` stanza pointing `catalyst` at the local
    //    path. Idempotent: if the section already exists we leave it alone and append our entry.
    patch_cargo_toml_for_local_catalyst(&project_dir, &catalyst_path);

    // 4. Manually author a minimal `users` resource via the typed state API. Doing this through the TUI (`blast gen resource users`) would require driving stdin keystrokes — out of scope for a smoke test.
    let state_dir = project_dir.join("storage").join("blast").join("state");
    write_minimal_users_resource(&state_dir);

    // 5. blast gen all (full pipeline).
    let gen_out = run_blast(blast_bin, &project_dir, &["gen", "all"]);
    assert_step_succeeded("blast gen all", &gen_out);

    // 6. cargo build (verify generated code compiles).
    let build_out = run_cmd(Command::new("cargo").arg("build").current_dir(&project_dir));
    assert_step_succeeded("cargo build", &build_out);

    // tempdir's Drop handles cleanup automatically; no explicit teardown
    // beyond letting `tmp` go out of scope.
}

/// Run a command and capture stdout/stderr without inheriting parent's
/// stdin (so any interactive prompt the child issues falls through).
fn run_cmd(cmd: &mut Command) -> Output {
    cmd.stdin(std::process::Stdio::null()).output().expect("spawn child process")
}

/// Convenience: run the blast binary with the given args in the given cwd.
fn run_blast(bin: &str, cwd: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new(bin);
    cmd.args(args).current_dir(cwd);
    run_cmd(&mut cmd)
}

/// Assert a child process exited 0; on failure dump everything captured
/// so debugging the e2e is straightforward.
fn assert_step_succeeded(label: &str, out: &Output) {
    if !out.status.success() {
        panic!(
            "step `{label}` failed (status: {status})\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
            label = label,
            status = out.status,
            stdout = String::from_utf8_lossy(&out.stdout),
            stderr = String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// Append a `[patch.crates-io]` stanza (if not already present) pointing
/// `catalyst` at the local checkout. Lets the scaffolded app build
/// against the catalyst we develop against, instead of whatever the
/// template Cargo.toml declared as the dependency source.
///
/// Stopgap until `blast new --dev` is reworked to scaffold a thin app
/// that depends on the published catalyst crate (today's `--dev` template
/// IS the legacy catalyst, so this patch is essentially a no-op pending
/// the template rewrite — see TODO in the file header).
fn patch_cargo_toml_for_local_catalyst(project_dir: &Path, catalyst_path: &Path) {
    let cargo_toml = project_dir.join("Cargo.toml");
    let original = std::fs::read_to_string(&cargo_toml).expect("read scaffolded Cargo.toml");
    let catalyst_str = catalyst_path.to_str().expect("catalyst path is utf-8");
    let patch_stanza = format!("\n[patch.crates-io]\ncatalyst = {{ path = \"{}\" }}\n", catalyst_str);
    let patched = match original.contains("[patch.crates-io]") {
        true => original, // leave the existing stanza alone; smoke harness is best-effort
        false => format!("{}{}", original.trim_end(), patch_stanza),
    };
    std::fs::write(&cargo_toml, patched).expect("write patched Cargo.toml");
}

/// Build a minimal but well-formed `users` resource state file with the
/// five canonical verbs and write it to
/// `<state_dir>/resources/users.ron` via the typed save_resource API.
fn write_minimal_users_resource(state_dir: &Path) {
    let mut res = ResourceState {
        schema_version: state::resource::RESOURCE_SCHEMA_VERSION,
        name: ResourceName::new("users"),
        fields: IndexMap::new(),
        verbs: IndexMap::new(),
        ws_events: None,
        singular_override: None,
        soft_delete: None,
        relations: BTreeMap::new(),
        gen_level: blast::state::GenLevel::default(),
        list_layout: None,
        detail_layout: None,
        toggle_endpoint: None,
            live_topics: Vec::new(),
    };

    let mut id_variants = BTreeSet::new();
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
                    kind: Default::default(),
        },
    );

    let mut email_variants = BTreeSet::new();
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
                    kind: Default::default(),
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
            emit_rest_api: true,
            emit_html_page: true,
        },
    );
    for verb in [Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
        res.verbs.insert(
            verb,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );
    }

    state::save_resource(state_dir, &res).expect("write users.ron");
}
