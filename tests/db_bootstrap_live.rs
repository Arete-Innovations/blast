//! Live Postgres integration test for the DB bootstrap module.
//!
//! Skipped unless `$BLAST_TEST_DB_URL` is set. The URL must point at an
//! admin-capable user (CREATE / DROP DATABASE privileges) on a reachable
//! Postgres instance. The dbname segment is REPLACED at runtime — never
//! reuse a production DB name here, or you will lose data.
//!
//! Example:
//!   BLAST_TEST_DB_URL=postgres://postgres:postgres@localhost:5432/postgres \
//!     cargo test --test db_bootstrap_live -- --nocapture

use blast::{
    io::null::NullSink,
    project::db_bootstrap::{self, BootstrapArgs, DbAction, RealDbAdmin},
};

fn admin_url() -> Option<String> {
    std::env::var("BLAST_TEST_DB_URL").ok()
}

/// Scrub the dbname into a per-test sentinel so we never collide with
/// whatever lives on the URL the user pointed us at.
fn scrub_url(template: &str, dbname: &str) -> String {
    let parsed = db_bootstrap::parse_url(template).expect("parse template");
    parsed.with_dbname(dbname).rebuild()
}

#[test]
fn live_bootstrap_creates_and_drops_dbs() {
    let Some(template) = admin_url() else {
        eprintln!("skipping: BLAST_TEST_DB_URL not set");
        return;
    };
    let project = "blast_bootstrap_live_t1";
    let target_url = scrub_url(&template, project);

    // Pre-clean from any previous failed run.
    {
        let parsed = db_bootstrap::parse_url(&target_url).expect("parse");
        let admin_target = db_bootstrap::admin_url(&parsed);
        let mut admin = RealDbAdmin;
        let _ = <RealDbAdmin as db_bootstrap::DbAdmin>::drop_database(&mut admin, &admin_target, project);
        let _ = <RealDbAdmin as db_bootstrap::DbAdmin>::drop_database(&mut admin, &admin_target, &format!("{}_test", project));
    }

    let mut admin = RealDbAdmin;
    let mut sink = NullSink;
    let outcome = db_bootstrap::bootstrap(
        &BootstrapArgs {
            project_name: project.to_string(),
            db_url: target_url.clone(),
            force: false,
            no_test_db: false,
        },
        &mut admin,
        &mut sink,
    )
    .expect("bootstrap");

    assert_eq!(outcome.primary_action, DbAction::Created);
    assert_eq!(outcome.test_action, Some(DbAction::Created));
    assert_eq!(outcome.primary_url, target_url);

    // Re-running without --force should refuse... wait, primary is empty so
    // it gets reused. Confirm that path.
    let outcome2 = db_bootstrap::bootstrap(
        &BootstrapArgs {
            project_name: project.to_string(),
            db_url: target_url.clone(),
            force: false,
            no_test_db: true,
        },
        &mut admin,
        &mut sink,
    )
    .expect("re-bootstrap empty db");
    assert_eq!(outcome2.primary_action, DbAction::Reused);

    // Cleanup.
    let parsed = db_bootstrap::parse_url(&target_url).expect("parse");
    let admin_target = db_bootstrap::admin_url(&parsed);
    let _ = <RealDbAdmin as db_bootstrap::DbAdmin>::drop_database(&mut admin, &admin_target, project);
    let _ = <RealDbAdmin as db_bootstrap::DbAdmin>::drop_database(&mut admin, &admin_target, &format!("{}_test", project));
}

#[test]
fn live_bootstrap_fails_fast_on_unreachable_postgres() {
    if admin_url().is_none() {
        eprintln!("skipping: BLAST_TEST_DB_URL not set");
        return;
    }
    let mut admin = RealDbAdmin;
    let mut sink = NullSink;
    // Port 1 is reserved; nothing should answer.
    let err = db_bootstrap::bootstrap(
        &BootstrapArgs {
            project_name: "doesnt_matter".to_string(),
            db_url: "postgres://nobody:nobody@127.0.0.1:1/whatever".to_string(),
            force: false,
            no_test_db: true,
        },
        &mut admin,
        &mut sink,
    )
    .expect_err("must fail");
    let msg = format!("{}", err);
    assert!(msg.contains("could not connect to Postgres"), "msg = {}", msg);
}
