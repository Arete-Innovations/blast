//! Per-enum file emitter for SQL to Rust ENUM codegen.
//!
//! Discovers IR via the migration scanner, then emits one file per
//! enum under src/structs/generated/enums/ alongside a barrel mod.rs.
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
            scan::{existing_user_enums, scan_project_enums, ParsedEnum},
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

    let user_enums = existing_user_enums(project_root)?;
    let emit_targets: Vec<&ParsedEnum> = scan
        .enums
        .iter()
        .filter(|parsed| {
            let pascal = render::enum_type_name(&parsed.name);
            if user_enums.contains(&pascal) {
                sink.info(format!("enums: skipping {} (hand-written {} found in src/structs/)", parsed.name, pascal));
                false
            } else {
                true
            }
        })
        .collect();

    if emit_targets.is_empty() {
        sink.info(format!("{STEP_LABEL}: every CREATE TYPE has a hand-written enum; nothing to emit"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let out_dir = enums_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let total = emit_targets.len() as u64;
    for (idx, parsed) in emit_targets.iter().enumerate() {
        emit_enum(project_root, parsed, &out_dir, &mut report, sink)?;
        sink.info(format!("enums: emitted {}", enum_target(&out_dir, parsed).display()));
        progress.tick(idx as u64 + 1, total);
    }

    let barrel_target = out_dir.join("mod.rs");
    let barrel_marker = header::marker_for_schema(project_root)?;
    let barrel_body = format!("{}{}", barrel_marker, render_barrel(&emit_targets));
    write_file(&barrel_target, &barrel_body, &mut report)?;
    sink.info(format!("enums: emitted {}", barrel_target.display()));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn emit_enum(project_root: &Path, parsed: &ParsedEnum, out_dir: &Path, report: &mut EmitReport, sink: &mut dyn Sink) -> BlastResult<()> {
    let target = enum_target(out_dir, parsed);
    let marker = marker_for_enum(project_root, parsed)?;
    let state_dir = project_root.join("storage").join("blast").join("state");
    let meta = match crate::state::load_enum_meta(&state_dir, &parsed.name) {
        Ok(m) => m,
        Err(e) => {
            sink.warn(format!("enums: failed to load metadata for '{}' ({}); falling back to PascalCase labels", parsed.name, e));
            crate::state::enum_meta::EnumMeta::empty(&parsed.name)
        }
    };
    let body = format!("{}{}", marker, render::render_enum_file(parsed, &meta));
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

fn render_barrel(enums: &[&ParsedEnum]) -> String {
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
        assert!(!role_body.starts_with("// AUTO-GENERATED"), "no inline marker");
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

        assert!(body.contains("use crate::database::schema::sql_types::UserRole as UserRoleSqlType;"));
        assert!(body.contains("use crate::meltdown::*;"));
        assert!(body.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]"));
        assert!(body.contains("#[cfg_attr(not(target_arch = \"wasm32\"), derive(AsExpression, FromSqlRow))]"));
        assert!(body.contains("diesel(sql_type = UserRoleSqlType)"));
        assert!(body.contains("pub enum UserRole {"));
        assert!(body.contains("    Admin,"));
        assert!(body.contains("    Member,"));
        assert!(body.contains("UserRole::Admin => \"admin\""));
        assert!(body.contains("\"admin\" => Ok(UserRole::Admin)"));
        assert!(body.contains("impl FromSql<UserRoleSqlType, Pg> for UserRole"));
        assert!(body.contains("b\"admin\" => Ok(UserRole::Admin)"));
        assert!(body.contains("impl ToSql<UserRoleSqlType, Pg> for UserRole"));
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

    #[test]
    fn skips_emission_when_hand_written_enum_already_exists() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_schema_stub(root);
        write_migration(root, "2026-04-26-000001_users_and_sessions", "CREATE TYPE user_role AS ENUM ('admin', 'member');");

        let role_dir = root.join("src/structs/auth");
        fs::create_dir_all(&role_dir).expect("mkdir auth");
        fs::write(role_dir.join("role.rs"), "pub enum Role {\n    Admin,\n    Member,\n}\n").expect("write role.rs");

        let user_role_dir = root.join("src/structs/users");
        fs::create_dir_all(&user_role_dir).expect("mkdir users");
        fs::write(user_role_dir.join("kind.rs"), "pub enum UserRole {\n    Admin,\n    Member,\n}\n").expect("write kind.rs");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run ok");

        let user_role_path = root.join("src/structs/generated/enums/user_role.rs");
        assert!(!user_role_path.exists(), "codegen should skip emission when UserRole already exists");
        assert!(report.written.is_empty(), "no files should be written");
    }

    #[test]
    fn skip_detection_ignores_generated_subtree() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_schema_stub(root);
        write_migration(root, "2026-01-01-000001_a", "CREATE TYPE post_status AS ENUM ('draft', 'live');");

        let prior_gen = root.join("src/structs/generated/enums");
        fs::create_dir_all(&prior_gen).expect("mkdir prior gen");
        fs::write(prior_gen.join("post_status.rs"), "pub enum PostStatus {\n    Draft,\n    Live,\n}\n").expect("write prior gen");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run ok");

        let target = root.join("src/structs/generated/enums/post_status.rs");
        assert!(target.exists(), "codegen should still emit when only generated/ has the enum");
        assert!(!report.written.is_empty(), "expected at least one written file");
    }
}
