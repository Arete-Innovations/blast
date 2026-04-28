//! Per-enum file emitter for SQL to Rust ENUM codegen.
//!
//! Pipeline shape mirrors the frontend_types runner: discover IR via
//! the migration scanner, then emit one file per enum under
//! src/structs/generated/enums/ alongside a barrel mod.rs.
//!
//! Output layout is flat-pack — enums are not per-resource. The barrel
//! re-exports each PascalCased enum name so callers can `use
//! crate::structs::generated::enums::UserRole;` without knowing which
//! file declared it.
//!
//! Each emitted file carries the standard auto-gen marker pointing at
//! the migration up.sql it was parsed from, so the user app build.rs
//! will refuse to compile if the migration changed since the last
//! blast gen.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{
        enums::{
            render,
            scan::{scan_project_enums, ParsedEnum},
        },
        header,
    },
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "enums: emit per-enum Rust types";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let scan = match scan_project_enums(project_root) {
        Ok(rep) => rep,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };

    for dup in &scan.duplicates {
        sink.warn(format!("enums: '{dup}' declared in multiple migrations; first occurrence wins"));
    }

    let mut report = EmitReport::default();

    if scan.enums.is_empty() {
        sink.info(format!("{STEP_LABEL}: no CREATE TYPE statements found; nothing to emit"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let out_dir = enums_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let total = scan.enums.len() as u64;
    for (idx, parsed) in scan.enums.iter().enumerate() {
        emit_enum(project_root, parsed, &out_dir, &mut report)?;
        sink.info(format!("enums: emitted {}", enum_target(&out_dir, parsed).display()));
        progress.tick(idx as u64 + 1, total);
    }

    let barrel_target = out_dir.join("mod.rs");
    let barrel_marker = header::marker_for_schema(project_root)?;
    let barrel_body = format!("{}{}", barrel_marker, render_barrel(&scan.enums));
    write_file(&barrel_target, &barrel_body, &mut report)?;
    sink.info(format!("enums: emitted {}", barrel_target.display()));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn emit_enum(project_root: &Path, parsed: &ParsedEnum, out_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let target = enum_target(out_dir, parsed);
    let marker = marker_for_enum(project_root, parsed)?;
    let body = format!("{}{}", marker, render::render_enum_file(parsed));
    write_file(&target, &body, report)
}

fn marker_for_enum(project_root: &Path, parsed: &ParsedEnum) -> BlastResult<String> {
    header::marker_for_state_file(project_root, &parsed.source_file)
}

fn enum_target(out_dir: &Path, parsed: &ParsedEnum) -> PathBuf {
    out_dir.join(format!("{}.rs", parsed.name))
}

fn enums_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("structs").join("generated").join("enums")
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("enums target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;

    match fs::read_to_string(target) {
        Ok(prev) if prev == body => {
            report.skipped.push(target.to_path_buf());
            return Ok(());
        }
        Ok(_different) => {} // allow: existing file differs, fall through to overwrite
        Err(_missing) => {}  // allow: file does not yet exist, fall through to write
    }

    fs::write(target, body)?;
    report.written.push(target.to_path_buf());
    Ok(())
}

fn render_barrel(enums: &[ParsedEnum]) -> String {
    let mut names: Vec<&str> = enums.iter().map(|e| e.name.as_str()).collect();
    names.sort();

    let mut out = String::new();
    for name in &names {
        out.push_str(&format!("pub mod {name};\n"));
    }
    out.push('\n');
    for name in &names {
        let type_name = render::enum_type_name(name);
        out.push_str(&format!("pub use {name}::{type_name};\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;
    use crate::io::null::{NullProgress, NullSink};

    fn write_migration(root: &Path, dir: &str, body: &str) {
        let mig = root.join("src/database/migrations").join(dir);
        fs::create_dir_all(&mig).expect("mkdir migration");
        let mut f = fs::File::create(mig.join("up.sql")).expect("create up.sql");
        f.write_all(body.as_bytes()).expect("write up.sql");
    }

    fn write_schema_stub(root: &Path) {
        let dir = root.join("src/database");
        fs::create_dir_all(&dir).expect("mkdir db");
        fs::write(dir.join("schema.rs"), "// placeholder\n").expect("write schema");
    }

    #[test]
    fn emits_nothing_when_no_migrations_exist() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_schema_stub(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run ok");
        assert!(report.written.is_empty());
        assert!(!root.join("src/structs/generated/enums").exists());
    }

    #[test]
    fn emits_per_enum_file_and_barrel() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_schema_stub(root);
        write_migration(root, "2026-01-01-000001_a", "CREATE TYPE user_role AS ENUM ('admin','member');");
        write_migration(root, "2026-01-02-000002_b", "CREATE TYPE post_status AS ENUM ('draft','live');");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run ok");

        let role_path = root.join("src/structs/generated/enums/user_role.rs");
        let post_path = root.join("src/structs/generated/enums/post_status.rs");
        let barrel = root.join("src/structs/generated/enums/mod.rs");
        assert!(role_path.exists());
        assert!(post_path.exists());
        assert!(barrel.exists());
        assert!(report.written.contains(&role_path));
        assert!(report.written.contains(&post_path));
        assert!(report.written.contains(&barrel));

        let role_body = fs::read_to_string(&role_path).expect("read role");
        assert!(role_body.starts_with("// AUTO-GENERATED from "));
        assert!(role_body.contains("pub enum UserRole {"));

        let barrel_body = fs::read_to_string(&barrel).expect("read barrel");
        assert!(barrel_body.contains("pub mod user_role;"));
        assert!(barrel_body.contains("pub mod post_status;"));
        assert!(barrel_body.contains("pub use user_role::UserRole;"));
        assert!(barrel_body.contains("pub use post_status::PostStatus;"));
    }

    #[test]
    fn renders_role_shape_matching_phase_1_canonical_fixture() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_schema_stub(root);
        write_migration(
            root,
            "2026-04-26-000001_users_and_sessions",
            "CREATE TYPE user_role AS ENUM ('admin', 'member');\nCREATE TABLE users (id BIGSERIAL PRIMARY KEY, role user_role NOT NULL DEFAULT 'member');",
        );

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run ok");

        let role_path = root.join("src/structs/generated/enums/user_role.rs");
        let body = fs::read_to_string(&role_path).expect("read user_role.rs");

        assert!(body.contains("use crate::database::schema::sql_types::UserRole;"));
        assert!(body.contains("use crate::meltdown::*;"));
        assert!(body.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsExpression, FromSqlRow, Serialize, Deserialize)]"));
        assert!(body.contains("#[diesel(sql_type = UserRole)]"));
        assert!(body.contains("pub enum UserRole {"));
        assert!(body.contains("    Admin,"));
        assert!(body.contains("    Member,"));
        assert!(body.contains("UserRole::Admin => \"admin\""));
        assert!(body.contains("\"admin\" => Ok(UserRole::Admin)"));
        assert!(body.contains("impl FromSql<UserRole, Pg> for UserRole"));
        assert!(body.contains("b\"admin\" => Ok(UserRole::Admin)"));
        assert!(body.contains("impl ToSql<UserRole, Pg> for UserRole"));
    }

    #[test]
    fn idempotent_second_run_skips_unchanged() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_schema_stub(root);
        write_migration(root, "2026-01-01-000001_a", "CREATE TYPE x AS ENUM ('a','b');");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _first = run(root, &mut sink, &mut progress).expect("first");
        let second = run(root, &mut sink, &mut progress).expect("second");

        assert!(second.written.is_empty());
        assert!(!second.skipped.is_empty());
    }
}
