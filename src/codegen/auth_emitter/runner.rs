//! Orchestration + file I/O for the auth emitter.
//!
//! Auth is special: its verbs (login/register/logout/me) are framework-fixed
//! rather than primer-declared, so it can't ride the per-resource emitter
//! pipeline. This runner:
//!
//! 1. Writes every fixed-shape auth file from the `templates` submodule,
//!    prefixed with the standard codegen marker keyed to the app state
//!    file (auth has no per-resource primer of its own).
//! 2. Idempotently extends the barrel `mod.rs` files emitted earlier in the
//!    pipeline by the resource-driven passes so the auth submodules and
//!    the flat `users` modules are pulled into the crate's module tree.
//!
//! Barrel mutation policy: append-only, idempotent (substring check before
//! adding any line). Existing resource-driven entries are preserved
//! verbatim. The runner does NOT sort or otherwise rewrite the barrel
//! content the prior emitter produced — auth lines are appended after.
//! On second invocation the substring checks short-circuit so the file
//! ends up byte-stable.
//!
//! `users.ron` primer interaction. If the user ships their own primer at
//! `storage/blast/state/resources/users.ron`, the auth emitter:
//! - skips writing `structs/generated/users.rs` and `models/generated/users.rs`
//!   (the resource-driven passes own them)
//! - still emits the auth-specific flows/routines/handlers/pages, which
//!   reference `crate::models::generated::users::{find_by_email, find_by_id, insert_new}`
//! The standard primer-driven models emitter only generates list/get/create/
//! update/delete, NOT the auth-needed lookup helpers — user takes ownership
//! by adding `models/users/` (user-owned, top-level subdir) with the
//! missing functions, or removes the primer to get the canonical baseline
//! back. Compile errors at the unresolved imports are the safety net.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{auth_emitter::templates, header},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
};

/// True when the project ships a `users.ron` primer — in that case the
/// resource-driven structs/models emitters own `users.rs` and the auth
/// emitter must not stomp them.
fn has_users_primer(project_root: &Path) -> bool {
    let primer = project_root.join("storage").join("blast").join("state").join("resources").join("users.ron");
    primer.is_file()
}

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "auth: emit fixed-shape auth files";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let mut report = EmitReport::default();
    let marker = header::marker_for_app(project_root)?;

    emit_structs(project_root, &marker, &mut report)?;
    emit_models(project_root, &marker, &mut report)?;
    emit_routines(project_root, &marker, &mut report)?;
    emit_flows(project_root, &marker, &mut report)?;
    emit_http(project_root, &marker, &mut report)?;
    emit_leptos_pages(project_root, &marker, &mut report)?;

    extend_structs_barrel(project_root, &marker, &mut report)?;
    extend_models_barrel(project_root, &marker, &mut report)?;
    extend_routines_barrel(project_root, &marker, &mut report)?;
    extend_flows_barrel(project_root, &marker, &mut report)?;
    extend_http_barrel(project_root, &marker, &mut report)?;
    extend_leptos_pages_barrel(project_root, &marker, &mut report)?;

    sink.info(format!("auth: {} file(s) written, {} skipped", report.written.len(), report.skipped.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

// ── per-domain emitters ───────────────────────────────────────────────────

fn emit_structs(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let structs_gen = project_root.join("src").join("structs").join("generated");
    if !has_users_primer(project_root) {
        write_file(&structs_gen.join("users.rs"), &format!("{marker}{}", templates::STRUCTS_USERS), report)?;
    }

    let auth_dir = structs_gen.join("auth");
    write_file(&auth_dir.join("login.rs"), &format!("{marker}{}", templates::STRUCTS_AUTH_LOGIN), report)?;
    write_file(&auth_dir.join("register.rs"), &format!("{marker}{}", templates::STRUCTS_AUTH_REGISTER), report)?;
    write_file(&auth_dir.join("mod.rs"), &format!("{marker}{}", templates::STRUCTS_AUTH_MOD), report)?;
    Ok(())
}

fn emit_models(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    if has_users_primer(project_root) {
        return Ok(());
    }
    let models_gen = project_root.join("src").join("models").join("generated");
    write_file(&models_gen.join("users.rs"), &format!("{marker}{}", templates::MODELS_USERS), report)?;
    Ok(())
}

fn emit_routines(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let auth_dir = project_root.join("src").join("routines").join("generated").join("auth");
    write_file(&auth_dir.join("login.rs"), &format!("{marker}{}", templates::ROUTINES_AUTH_LOGIN), report)?;
    write_file(&auth_dir.join("register.rs"), &format!("{marker}{}", templates::ROUTINES_AUTH_REGISTER), report)?;
    write_file(&auth_dir.join("logout.rs"), &format!("{marker}{}", templates::ROUTINES_AUTH_LOGOUT), report)?;
    write_file(&auth_dir.join("me.rs"), &format!("{marker}{}", templates::ROUTINES_AUTH_ME), report)?;
    write_file(&auth_dir.join("mod.rs"), &format!("{marker}{}", templates::ROUTINES_AUTH_MOD), report)?;
    Ok(())
}

fn emit_flows(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let auth_dir = project_root.join("src").join("flows").join("generated").join("auth");
    write_file(&auth_dir.join("login.rs"), &format!("{marker}{}", templates::FLOWS_AUTH_LOGIN), report)?;
    write_file(&auth_dir.join("register.rs"), &format!("{marker}{}", templates::FLOWS_AUTH_REGISTER), report)?;
    write_file(&auth_dir.join("logout.rs"), &format!("{marker}{}", templates::FLOWS_AUTH_LOGOUT), report)?;
    write_file(&auth_dir.join("me.rs"), &format!("{marker}{}", templates::FLOWS_AUTH_ME), report)?;
    write_file(&auth_dir.join("mod.rs"), &format!("{marker}{}", templates::FLOWS_AUTH_MOD), report)?;
    Ok(())
}

fn emit_http(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let http_gen = project_root.join("src").join("transport").join("http").join("generated");
    write_file(&http_gen.join("auth.rs"), &format!("{marker}{}", templates::HTTP_AUTH), report)?;
    Ok(())
}

fn emit_leptos_pages(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let pages_gen = project_root.join("src").join("transport").join("leptos").join("pages").join("generated");
    write_file(&pages_gen.join("login.rs"), &format!("{marker}{}", templates::LEPTOS_LOGIN), report)?;
    write_file(&pages_gen.join("register.rs"), &format!("{marker}{}", templates::LEPTOS_REGISTER), report)?;
    write_file(&pages_gen.join("logout.rs"), &format!("{marker}{}", templates::LEPTOS_LOGOUT), report)?;
    write_file(&pages_gen.join("profile.rs"), &format!("{marker}{}", templates::LEPTOS_PROFILE), report)?;
    // profile.module.scss carries no marker — stylance/grass don't grok the rust comment header.
    write_file(&pages_gen.join("profile.module.scss"), templates::LEPTOS_PROFILE_SCSS, report)?;
    Ok(())
}

// ── barrel extensions ─────────────────────────────────────────────────────

/// Required entries in `src/structs/generated/mod.rs` (post-merge):
/// `pub mod auth;`, `pub mod enums;`, plus — when no user-supplied
/// `users.ron` primer exists — `pub mod users;`, `pub use enums::UserRole;`,
/// `pub use users::UserPublic;`, and a wasm-cfg-gated re-export of
/// `NewUser`/`User` (diesel-derives don't compile to wasm32).
///
/// The `validators` line is owned by the validators pass
/// (`ensure_parent_structs_barrel_includes_validators`). The enums pass
/// emits its own `enums/mod.rs` but does NOT touch the parent barrel —
/// auth depends on `crate::structs::generated::UserRole` so the parent
/// barrel must `pub mod enums; pub use enums::UserRole;`. When a primer
/// drives `users.rs` the user's fields and projection types don't have a
/// `NewUser`, so the canonical re-exports get skipped to avoid an
/// undefined-name compile failure.
fn extend_structs_barrel(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src").join("structs").join("generated").join("mod.rs");
    let mut entries: Vec<&str> = vec!["pub mod auth;", "pub mod enums;"];
    if !has_users_primer(project_root) {
        entries.push("pub mod users;");
        entries.push("pub use enums::UserRole;");
        entries.push("pub use users::UserPublic;");
        entries.push("#[cfg(not(target_arch = \"wasm32\"))]");
        entries.push("pub use users::{NewUser, User};");
    }
    append_lines_idempotent(&path, marker, &entries, report)
}

/// `models/generated/mod.rs` needs `pub mod users;` only when the auth
/// emitter is the one writing `users.rs`. If a `users.ron` primer exists
/// the resource-driven models pass already wrote `pub mod users;` to the
/// barrel before us — substring check would short-circuit, so this is
/// idempotent either way; the explicit guard keeps intent clear.
fn extend_models_barrel(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    if has_users_primer(project_root) {
        return Ok(());
    }
    let path = project_root.join("src").join("models").join("generated").join("mod.rs");
    append_lines_idempotent(&path, marker, &["pub mod users;"], report)
}

fn extend_routines_barrel(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src").join("routines").join("generated").join("mod.rs");
    append_lines_idempotent(&path, marker, &["pub mod auth;"], report)
}

fn extend_flows_barrel(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src").join("flows").join("generated").join("mod.rs");
    append_lines_idempotent(&path, marker, &["pub mod auth;"], report)
}

fn extend_http_barrel(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src").join("transport").join("http").join("generated").join("mod.rs");
    append_lines_idempotent(&path, marker, &["pub mod auth;"], report)
}

/// Leptos pages barrel: flat `pub mod login;` etc. plus the `pub use`
/// re-exports for each *Page component. Resource-driven pages have a per-
/// table subdir (`pub mod posts;`) that the leptos_pages pass adds; auth
/// pages are flat siblings.
fn extend_leptos_pages_barrel(project_root: &Path, marker: &str, report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src").join("transport").join("leptos").join("pages").join("generated").join("mod.rs");
    let mut entries = vec![
        "pub mod login;",
        "pub mod logout;",
        "pub mod profile;",
        "pub mod register;",
        "pub use login::LoginPage;",
        "pub use logout::LogoutPage;",
        "pub use profile::ProfilePage;",
        "pub use register::RegisterPage;",
    ];
    entries.sort();
    append_lines_idempotent(&path, marker, &entries, report)
}

// ── helpers ───────────────────────────────────────────────────────────────

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("auth target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;

    if target.exists() {
        let prev = fs::read_to_string(target)?;
        if prev == body {
            report.skipped.push(target.to_path_buf());
            return Ok(());
        }
    }

    let mut file = fs::File::create(target)?;
    file.write_all(body.as_bytes())?;
    report.written.push(target.to_path_buf());
    Ok(())
}

/// Append each entry to the barrel at `path` if not already present as a
/// substring. Lines are joined with `\n`. If the file does not exist it is
/// created with the marker header followed by the lines.
///
/// Substring (not line-equality) is used because earlier passes write
/// without a trailing newline on the last entry; appending ensures
/// idempotency without depending on the precise terminator.
fn append_lines_idempotent(path: &Path, marker: &str, entries: &[&str], report: &mut EmitReport) -> BlastResult<()> {
    let parent = path.parent().ok_or_else(|| BlastError::Invalid(format!("barrel target has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;

    let existing: Option<String> = match fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None, // allow: NotFound means no prior barrel — append fresh
        Err(e) => return Err(BlastError::from(e)),
    };

    let mut needed: Vec<&str> = Vec::new();
    for entry in entries {
        let already = match &existing {
            Some(prev) => prev.lines().any(|l| l.trim() == entry.trim()),
            None => false, // allow: no prior file means entry is not yet present
        };
        if !already {
            needed.push(entry);
        }
    }

    if needed.is_empty() && existing.is_some() {
        report.skipped.push(path.to_path_buf());
        return Ok(());
    }

    let body = match existing {
        Some(prev) => {
            if needed.is_empty() {
                prev
            } else {
                let mut buf = prev;
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
                for line in &needed {
                    buf.push_str(line);
                    buf.push('\n');
                }
                buf
            }
        }
        None => {
            let mut buf = String::from(marker);
            for line in &needed {
                buf.push_str(line);
                buf.push('\n');
            }
            buf
        }
    };

    fs::write(path, &body)?;
    report.written.push(path.to_path_buf());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs as stdfs;

    use tempfile::TempDir;

    use super::*;
    use crate::io::null::{NullProgress, NullSink};

    fn seed_app_state(root: &Path) {
        let state_dir = root.join("storage").join("blast").join("state");
        stdfs::create_dir_all(&state_dir).expect("mkdir state");
        let app = crate::state::AppState::default();
        crate::state::io::save_app(&state_dir, &app).expect("save app");
    }

    #[test]
    fn emits_all_auth_files_with_markers() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_app_state(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("auth codegen ok");

        let expected_files = [
            "src/structs/generated/users.rs",
            "src/structs/generated/auth/login.rs",
            "src/structs/generated/auth/register.rs",
            "src/structs/generated/auth/mod.rs",
            "src/models/generated/users.rs",
            "src/routines/generated/auth/login.rs",
            "src/routines/generated/auth/register.rs",
            "src/routines/generated/auth/logout.rs",
            "src/routines/generated/auth/me.rs",
            "src/routines/generated/auth/mod.rs",
            "src/flows/generated/auth/login.rs",
            "src/flows/generated/auth/register.rs",
            "src/flows/generated/auth/logout.rs",
            "src/flows/generated/auth/me.rs",
            "src/flows/generated/auth/mod.rs",
            "src/transport/http/generated/auth.rs",
            "src/transport/leptos/pages/generated/login.rs",
            "src/transport/leptos/pages/generated/register.rs",
            "src/transport/leptos/pages/generated/logout.rs",
            "src/transport/leptos/pages/generated/profile.rs",
            "src/transport/leptos/pages/generated/profile.module.scss",
        ];

        for rel in expected_files {
            let p = root.join(rel);
            assert!(p.exists(), "missing emitted file: {}", p.display());
        }
        assert!(!report.written.is_empty());

        let body = stdfs::read_to_string(root.join("src/structs/generated/users.rs")).expect("read users.rs");
        assert!(body.starts_with("// AUTO-GENERATED from "), "missing marker: {body}");
        assert!(body.contains("pub struct User {"));
        assert!(body.contains("pub struct NewUser {"));
        assert!(body.contains("pub struct UserPublic {"));

        let scss = stdfs::read_to_string(root.join("src/transport/leptos/pages/generated/profile.module.scss")).expect("read scss");
        assert!(!scss.starts_with("// AUTO-GENERATED"), "scss must NOT carry rust comment marker (grass would choke): {scss}");
    }

    #[test]
    fn idempotent_second_run_skips_unchanged_files() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_app_state(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _first = run(root, &mut sink, &mut progress).expect("first run");
        let second = run(root, &mut sink, &mut progress).expect("second run");
        assert!(!second.skipped.is_empty(), "second run must skip identical files: {:?}", second);
    }

    #[test]
    fn extends_existing_barrel_idempotently() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_app_state(root);

        let routines_barrel = root.join("src/routines/generated/mod.rs");
        stdfs::create_dir_all(routines_barrel.parent().unwrap()).expect("mkdir");
        stdfs::write(&routines_barrel, "// AUTO-GENERATED from storage/blast/state/app.ron @ deadbeef\n//\n// Do not edit by hand.\n\npub mod posts;\n").expect("seed barrel");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("first run");
        let body = stdfs::read_to_string(&routines_barrel).expect("read barrel");
        assert!(body.contains("pub mod posts;"), "must preserve existing entry");
        assert!(body.contains("pub mod auth;"), "must add auth entry");

        run(root, &mut sink, &mut progress).expect("second run");
        let body2 = stdfs::read_to_string(&routines_barrel).expect("read barrel");
        let auth_count = body2.matches("pub mod auth;").count();
        assert_eq!(auth_count, 1, "second run must not duplicate auth entry: {body2}");
    }

    #[test]
    fn structs_barrel_includes_use_reexports() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_app_state(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let body = stdfs::read_to_string(root.join("src/structs/generated/mod.rs")).expect("read structs barrel");
        assert!(body.contains("pub mod auth;"), "structs barrel must declare auth: {body}");
        assert!(body.contains("pub mod enums;"), "structs barrel must declare enums (for UserRole): {body}");
        assert!(body.contains("pub mod users;"), "structs barrel must declare users: {body}");
        assert!(body.contains("pub use users::UserPublic;"), "structs barrel must re-export UserPublic (wasm-friendly): {body}");
        assert!(body.contains("pub use users::{NewUser, User};"), "structs barrel must re-export native-only user types: {body}");
        assert!(body.contains("#[cfg(not(target_arch = \"wasm32\"))]"), "structs barrel must cfg-gate native types: {body}");
        assert!(body.contains("pub use enums::UserRole;"), "structs barrel must re-export UserRole: {body}");
    }

    #[test]
    fn skips_users_files_when_primer_present() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_app_state(root);

        let resources_dir = root.join("storage/blast/state/resources");
        stdfs::create_dir_all(&resources_dir).expect("mkdir resources");
        stdfs::write(resources_dir.join("users.ron"), "(stub)\n").expect("seed users primer");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let users_struct = root.join("src/structs/generated/users.rs");
        let users_model = root.join("src/models/generated/users.rs");
        assert!(!users_struct.exists(), "users.rs must NOT be auth-emitted when primer exists: {}", users_struct.display());
        assert!(!users_model.exists(), "models/users.rs must NOT be auth-emitted when primer exists: {}", users_model.display());

        let structs_barrel = stdfs::read_to_string(root.join("src/structs/generated/mod.rs")).expect("read structs barrel");
        assert!(structs_barrel.contains("pub mod auth;"), "auth still required: {structs_barrel}");
        assert!(!structs_barrel.contains("pub use users::{NewUser, User};"), "must not re-export NewUser when primer drives users: {structs_barrel}");
    }

    #[test]
    fn leptos_pages_barrel_emits_flat_auth_modules_and_reexports() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_app_state(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let body = stdfs::read_to_string(root.join("src/transport/leptos/pages/generated/mod.rs")).expect("read pages barrel");
        for needle in ["pub mod login;", "pub mod logout;", "pub mod profile;", "pub mod register;", "pub use login::LoginPage;", "pub use logout::LogoutPage;", "pub use profile::ProfilePage;", "pub use register::RegisterPage;"] {
            assert!(body.contains(needle), "pages barrel missing {needle}: {body}");
        }
    }
}
