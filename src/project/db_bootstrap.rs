//! DB bootstrap for `blast new`.
//!
//! Fail-fast contract: before any project files are written, we MUST verify
//! that the user gave us a reachable Postgres URL and that the target
//! database is either empty / missing (we create it) or explicitly opted into
//! reuse via `--force` (we drop + recreate it). If anything goes wrong, the
//! user gets a clear error and **no files are written**. No half-broken
//! projects on disk.
//!
//! The actual DB I/O is hidden behind the `DbAdmin` trait so the orchestration
//! logic can be unit-tested with an in-memory fake. The real implementation
//! lives in `RealDbAdmin` and uses diesel's `PgConnection` + `sql_query`.

use diesel::{pg::PgConnection, prelude::*, sql_query, sql_types::BigInt, QueryableByName};

use crate::{
    error::{BlastError, BlastResult},
    io::traits::{Sink, SinkExt},
};

const ADMIN_DB: &str = "postgres";

/// Result of one DB lifecycle decision (created vs reused vs recreated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbAction {
    Created,
    Reused,
    Recreated,
}

/// Args for the bootstrap entry point. Mirrors the CLI args.
#[derive(Debug, Clone)]
pub struct BootstrapArgs {
    pub project_name: String,
    /// User-supplied URL (from `--db-url` or interactive prompt). Already
    /// resolved by the caller.
    pub db_url: String,
    pub force: bool,
    pub no_test_db: bool,
}

/// Outcome of the bootstrap step. Used by the scaffold layer to write the
/// `.env` / `.env.test` files with the correct URLs.
#[derive(Debug, Clone)]
pub struct BootstrapOutcome {
    pub primary_url: String,
    pub primary_action: DbAction,
    pub test_url: Option<String>,
    pub test_action: Option<DbAction>,
}

// ---------------------------------------------------------------------------
// URL parsing
// ---------------------------------------------------------------------------

/// Minimal parsed Postgres URL. We only need to extract / swap the dbname
/// segment; full URL parsing belongs to whatever consumes the connection
/// string downstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    /// Everything up to and including the last `/`, e.g.
    /// `postgres://user:pass@host:5432/`.
    pub prefix: String,
    /// The dbname segment (everything between the last `/` and `?` or end).
    pub dbname: String,
    /// Optional query string (including leading `?`), preserved verbatim.
    pub query: String,
}

impl ParsedUrl {
    pub fn rebuild(&self) -> String {
        format!("{}{}{}", self.prefix, self.dbname, self.query)
    }

    pub fn with_dbname(&self, name: &str) -> ParsedUrl {
        ParsedUrl {
            prefix: self.prefix.clone(),
            dbname: name.to_string(),
            query: self.query.clone(),
        }
    }
}

pub fn parse_url(url: &str) -> BlastResult<ParsedUrl> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(BlastError::Invalid("database URL is empty".to_string()));
    }
    if !trimmed.starts_with("postgres://") && !trimmed.starts_with("postgresql://") {
        return Err(BlastError::Invalid(format!("database URL must start with `postgres://` or `postgresql://`: `{}`", mask_for_error(trimmed))));
    }
    let scheme_end = trimmed.find("://").ok_or_else(|| BlastError::Invalid("malformed scheme".to_string()))? + 3;
    let after_scheme = &trimmed[scheme_end..];

    // dbname starts after the LAST `/` and runs until `?` or end. We look
    // only at characters after the scheme so we don't accidentally hit the
    // `://` slashes.
    let last_slash = after_scheme
        .rfind('/')
        .ok_or_else(|| BlastError::Invalid(format!("database URL has no dbname segment: `{}`", mask_for_error(trimmed))))?;

    let prefix_end = scheme_end + last_slash + 1; // include the slash
    let prefix = trimmed[..prefix_end].to_string();
    let tail = &trimmed[prefix_end..];
    let (dbname, query) = match tail.find('?') {
        Some(idx) => (tail[..idx].to_string(), tail[idx..].to_string()),
        None => (tail.to_string(), String::new()),
    };

    if dbname.is_empty() {
        return Err(BlastError::Invalid(format!("database URL has empty dbname: `{}`", mask_for_error(trimmed))));
    }

    Ok(ParsedUrl { prefix, dbname, query })
}

/// Swap the dbname to `postgres` (the default admin DB on every Postgres
/// installation) for CREATE / DROP / DATABASE existence checks.
pub fn admin_url(parsed: &ParsedUrl) -> String {
    parsed.with_dbname(ADMIN_DB).rebuild()
}

/// Derive the test-DB URL by appending `_test` to the dbname.
pub fn test_url(parsed: &ParsedUrl) -> ParsedUrl {
    let test_db = format!("{}_test", parsed.dbname);
    parsed.with_dbname(&test_db)
}

/// Default URL suggestion shown in the interactive prompt when the user did
/// not pass `--db-url`.
pub fn default_url_for(project_name: &str) -> String {
    format!("postgres://postgres:postgres@localhost:5432/{}", project_name)
}

fn mask_for_error(url: &str) -> String {
    // Don't leak the password back in error output.
    let scheme_end = match url.find("://") {
        Some(idx) => idx,
        None => return url.to_string(),
    };
    let head = &url[..scheme_end + 3];
    let at_idx = match url[scheme_end + 3..].find('@') {
        Some(idx) => idx,
        None => return url.to_string(),
    };
    let after_at = &url[scheme_end + 3 + at_idx..];
    format!("{}<masked>{}", head, after_at)
}

// ---------------------------------------------------------------------------
// DbAdmin trait + real impl
// ---------------------------------------------------------------------------

#[derive(Debug, QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

/// Thin trait so the orchestrator can be tested without a live Postgres.
/// Real implementation lives in `RealDbAdmin` below.
pub trait DbAdmin {
    /// Open a single connection to the URL to verify reachability + creds.
    /// Should NOT use the target dbname (use `admin_url` for ping).
    fn ping(&mut self, admin_url: &str) -> BlastResult<()>;

    /// Returns true iff a database with that name exists.
    fn db_exists(&mut self, admin_url: &str, dbname: &str) -> BlastResult<bool>;

    /// Returns the count of tables in the public schema of the target DB.
    /// Caller is responsible for connecting to the target DB (not admin).
    fn count_public_tables(&mut self, target_url: &str) -> BlastResult<i64>;

    fn create_database(&mut self, admin_url: &str, dbname: &str) -> BlastResult<()>;

    fn drop_database(&mut self, admin_url: &str, dbname: &str) -> BlastResult<()>;
}

pub struct RealDbAdmin;

impl DbAdmin for RealDbAdmin {
    fn ping(&mut self, admin_url: &str) -> BlastResult<()> {
        // `establish` already does a real handshake; no extra query needed.
        // We open + drop on the same line to make the side-effect explicit
        // and avoid a temp binding the linter would flag.
        open(admin_url)?;
        Ok(())
    }

    fn db_exists(&mut self, admin_url: &str, dbname: &str) -> BlastResult<bool> {
        let mut conn = open(admin_url)?;
        let q = format!("SELECT count(*) AS count FROM pg_database WHERE datname = '{}'", escape_sql_literal(dbname));
        let rows: Vec<CountRow> = sql_query(q).load(&mut conn)?;
        match rows.first() {
            Some(row) => Ok(row.count > 0),
            None => Err(BlastError::Project("pg_database count query returned zero rows".to_string())),
        }
    }

    fn count_public_tables(&mut self, target_url: &str) -> BlastResult<i64> {
        let mut conn = open(target_url)?;
        let rows: Vec<CountRow> = sql_query("SELECT count(*) AS count FROM information_schema.tables WHERE table_schema = 'public'").load(&mut conn)?;
        match rows.first() {
            Some(row) => Ok(row.count),
            None => Err(BlastError::Project("information_schema.tables count query returned zero rows".to_string())),
        }
    }

    fn create_database(&mut self, admin_url: &str, dbname: &str) -> BlastResult<()> {
        let mut conn = open(admin_url)?;
        let q = format!("CREATE DATABASE \"{}\"", escape_ident(dbname));
        sql_query(q).execute(&mut conn)?;
        Ok(())
    }

    fn drop_database(&mut self, admin_url: &str, dbname: &str) -> BlastResult<()> {
        let mut conn = open(admin_url)?;
        // Kill any leftover backends so DROP doesn't deadlock against a
        // forgotten psql session. Best-effort: failure here means there were
        // no backends to terminate or we lacked the privilege; either way,
        // the subsequent DROP DATABASE will report the real problem.
        let term = format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}' AND pid <> pg_backend_pid()",
            escape_sql_literal(dbname)
        );
        match sql_query(term).execute(&mut conn) {
            Ok(_n) => {}
            Err(_e) => {} // allow: best-effort backend kill before DROP DATABASE
        }

        let q = format!("DROP DATABASE IF EXISTS \"{}\"", escape_ident(dbname));
        sql_query(q).execute(&mut conn)?;
        Ok(())
    }
}

fn open(url: &str) -> BlastResult<PgConnection> {
    PgConnection::establish(url).map_err(|e| BlastError::Project(format!("could not connect to Postgres at `{}`: {}", mask_for_error(url), e)))
}

fn escape_ident(s: &str) -> String {
    s.replace('"', "\"\"")
}

fn escape_sql_literal(s: &str) -> String {
    s.replace('\'', "''")
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Run the full bootstrap sequence. Returns the resolved URLs so the
/// caller can write the `.env` / `.env.test` files.
///
/// Steps:
/// 1. parse the user-supplied URL
/// 2. ping the admin DB (fails fast if Postgres unreachable)
/// 3. ensure the primary DB is empty / created (or recreated under `--force`)
/// 4. same for the test DB unless `--no-test-db`
pub fn bootstrap(args: &BootstrapArgs, admin: &mut dyn DbAdmin, sink: &mut dyn Sink) -> BlastResult<BootstrapOutcome> {
    let parsed = parse_url(&args.db_url)?;
    let admin_target = admin_url(&parsed);

    sink.info(format!("verifying Postgres reachable at {}", mask_for_error(&parsed.rebuild())));
    admin.ping(&admin_target)?;

    let primary_action = ensure_clean_db(&parsed, args.force, admin, sink)?;

    let (test_url_str, test_action) = if args.no_test_db {
        (None, None)
    } else {
        let test_parsed = test_url(&parsed);
        let action = ensure_clean_db(&test_parsed, args.force, admin, sink)?;
        (Some(test_parsed.rebuild()), Some(action))
    };

    Ok(BootstrapOutcome {
        primary_url: parsed.rebuild(),
        primary_action,
        test_url: test_url_str,
        test_action,
    })
}

fn ensure_clean_db(parsed: &ParsedUrl, force: bool, admin: &mut dyn DbAdmin, sink: &mut dyn Sink) -> BlastResult<DbAction> {
    let admin_target = admin_url(parsed);
    let target_url = parsed.rebuild();
    let dbname = &parsed.dbname;

    let exists = admin.db_exists(&admin_target, dbname)?;
    if !exists {
        sink.info(format!("creating database `{}`", dbname));
        admin.create_database(&admin_target, dbname)?;
        return Ok(DbAction::Created);
    }

    // DB exists. Check if it's empty.
    let table_count = admin.count_public_tables(&target_url)?;
    if table_count == 0 {
        sink.info(format!("reusing empty database `{}`", dbname));
        return Ok(DbAction::Reused);
    }

    // DB exists with tables. Refuse without --force.
    if !force {
        return Err(BlastError::Project(format!(
            "database `{}` exists and has {} table(s). Was this a typo? Re-run with `--force` to drop and recreate, or pass `--db-url` with a different database name.",
            dbname, table_count
        )));
    }

    sink.warn(format!("--force: dropping and recreating database `{}` ({} table(s) destroyed)", dbname, table_count));
    admin.drop_database(&admin_target, dbname)?;
    admin.create_database(&admin_target, dbname)?;
    Ok(DbAction::Recreated)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::io::null::NullSink;

    #[test]
    fn parse_url_extracts_dbname() {
        let p = parse_url("postgres://u:p@localhost:5432/myapp").expect("parse");
        assert_eq!(p.dbname, "myapp");
        assert_eq!(p.prefix, "postgres://u:p@localhost:5432/");
        assert_eq!(p.query, "");
        assert_eq!(p.rebuild(), "postgres://u:p@localhost:5432/myapp");
    }

    #[test]
    fn parse_url_preserves_query_string() {
        let p = parse_url("postgres://u:p@h/myapp?sslmode=disable").expect("parse");
        assert_eq!(p.dbname, "myapp");
        assert_eq!(p.query, "?sslmode=disable");
        assert_eq!(p.rebuild(), "postgres://u:p@h/myapp?sslmode=disable");
    }

    #[test]
    fn parse_url_accepts_postgresql_scheme() {
        let p = parse_url("postgresql://u:p@h/db").expect("parse");
        assert_eq!(p.dbname, "db");
    }

    #[test]
    fn parse_url_rejects_empty() {
        assert!(parse_url("").is_err());
    }

    #[test]
    fn parse_url_rejects_wrong_scheme() {
        assert!(parse_url("mysql://u:p@h/db").is_err());
    }

    #[test]
    fn parse_url_rejects_missing_dbname() {
        assert!(parse_url("postgres://u:p@h/").is_err());
    }

    #[test]
    fn admin_url_swaps_to_postgres_db() {
        let p = parse_url("postgres://u:p@h/myapp").expect("parse");
        assert_eq!(admin_url(&p), "postgres://u:p@h/postgres");
    }

    #[test]
    fn test_url_appends_underscore_test() {
        let p = parse_url("postgres://u:p@h/myapp").expect("parse");
        let t = test_url(&p);
        assert_eq!(t.dbname, "myapp_test");
        assert_eq!(t.rebuild(), "postgres://u:p@h/myapp_test");
    }

    #[test]
    fn default_url_uses_project_name() {
        assert_eq!(default_url_for("acme"), "postgres://postgres:postgres@localhost:5432/acme");
    }

    #[test]
    fn mask_for_error_redacts_password() {
        assert_eq!(mask_for_error("postgres://user:secret@host/db"), "postgres://<masked>@host/db");
    }

    // ---- fake admin for orchestrator tests --------------------------------

    #[derive(Default)]
    struct FakeAdmin {
        // Map of dbname -> table count. Presence in map = exists.
        databases: HashMap<String, i64>,
        ping_should_fail: bool,
        ping_count: usize,
        creates: Vec<String>,
        drops: Vec<String>,
    }

    impl DbAdmin for FakeAdmin {
        fn ping(&mut self, _admin_url: &str) -> BlastResult<()> {
            self.ping_count += 1;
            if self.ping_should_fail {
                Err(BlastError::Project("ping failed".to_string()))
            } else {
                Ok(())
            }
        }

        fn db_exists(&mut self, _admin_url: &str, dbname: &str) -> BlastResult<bool> {
            Ok(self.databases.contains_key(dbname))
        }

        fn count_public_tables(&mut self, target_url: &str) -> BlastResult<i64> {
            // target_url ends in /<dbname>; pull it back out.
            let parsed = parse_url(target_url)?;
            Ok(self.databases.get(&parsed.dbname).copied().unwrap_or(0))
        }

        fn create_database(&mut self, _admin_url: &str, dbname: &str) -> BlastResult<()> {
            self.creates.push(dbname.to_string());
            self.databases.insert(dbname.to_string(), 0);
            Ok(())
        }

        fn drop_database(&mut self, _admin_url: &str, dbname: &str) -> BlastResult<()> {
            self.drops.push(dbname.to_string());
            self.databases.remove(dbname);
            Ok(())
        }
    }

    fn args(name: &str, force: bool, no_test_db: bool) -> BootstrapArgs {
        BootstrapArgs {
            project_name: name.to_string(),
            db_url: format!("postgres://u:p@h/{}", name),
            force,
            no_test_db,
        }
    }

    #[test]
    fn bootstrap_creates_missing_db() {
        let mut admin = FakeAdmin::default();
        let mut sink = NullSink;
        let outcome = bootstrap(&args("acme", false, true), &mut admin, &mut sink).expect("ok");
        assert_eq!(outcome.primary_action, DbAction::Created);
        assert_eq!(outcome.test_action, None);
        assert_eq!(admin.creates, vec!["acme".to_string()]);
        assert!(admin.drops.is_empty());
    }

    #[test]
    fn bootstrap_creates_test_db_too() {
        let mut admin = FakeAdmin::default();
        let mut sink = NullSink;
        let outcome = bootstrap(&args("acme", false, false), &mut admin, &mut sink).expect("ok");
        assert_eq!(outcome.primary_action, DbAction::Created);
        assert_eq!(outcome.test_action, Some(DbAction::Created));
        assert_eq!(outcome.test_url.as_deref(), Some("postgres://u:p@h/acme_test"));
        assert_eq!(admin.creates, vec!["acme".to_string(), "acme_test".to_string()]);
    }

    #[test]
    fn bootstrap_reuses_empty_db() {
        let mut admin = FakeAdmin::default();
        admin.databases.insert("acme".to_string(), 0);
        let mut sink = NullSink;
        let outcome = bootstrap(&args("acme", false, true), &mut admin, &mut sink).expect("ok");
        assert_eq!(outcome.primary_action, DbAction::Reused);
        assert!(admin.creates.is_empty());
    }

    #[test]
    fn bootstrap_refuses_populated_db_without_force() {
        let mut admin = FakeAdmin::default();
        admin.databases.insert("acme".to_string(), 7);
        let mut sink = NullSink;
        let err = bootstrap(&args("acme", false, true), &mut admin, &mut sink).expect_err("must fail");
        let msg = format!("{}", err);
        assert!(msg.contains("`acme`"), "msg = {}", msg);
        assert!(msg.contains("--force"), "msg = {}", msg);
        assert!(admin.creates.is_empty());
        assert!(admin.drops.is_empty());
    }

    #[test]
    fn bootstrap_recreates_populated_db_with_force() {
        let mut admin = FakeAdmin::default();
        admin.databases.insert("acme".to_string(), 7);
        let mut sink = NullSink;
        let outcome = bootstrap(&args("acme", true, true), &mut admin, &mut sink).expect("ok");
        assert_eq!(outcome.primary_action, DbAction::Recreated);
        assert_eq!(admin.drops, vec!["acme".to_string()]);
        assert_eq!(admin.creates, vec!["acme".to_string()]);
    }

    #[test]
    fn bootstrap_fails_fast_on_unreachable_postgres() {
        let mut admin = FakeAdmin {
            ping_should_fail: true,
            ..Default::default()
        };
        let mut sink = NullSink;
        let err = bootstrap(&args("acme", false, false), &mut admin, &mut sink).expect_err("must fail");
        assert!(format!("{}", err).contains("ping failed"));
        // Crucially: nothing was created.
        assert!(admin.creates.is_empty());
    }

    #[test]
    fn bootstrap_test_db_independently_force_recreates() {
        let mut admin = FakeAdmin::default();
        // primary missing, test populated.
        admin.databases.insert("acme_test".to_string(), 3);
        let mut sink = NullSink;
        let outcome = bootstrap(&args("acme", true, false), &mut admin, &mut sink).expect("ok");
        assert_eq!(outcome.primary_action, DbAction::Created);
        assert_eq!(outcome.test_action, Some(DbAction::Recreated));
    }
}
