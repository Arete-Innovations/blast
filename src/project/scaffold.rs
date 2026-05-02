//! Project scaffolder.
//!
//! `blast new <name>` and `blast init [<name>]` both land here. The
//! scaffolder vendors the entire Catalyst framework as files baked into
//! the blast binary via `include_dir!`. Scaffolded apps DO NOT depend on
//! a separate `catalyst` crate — the framework code IS the user app's
//! own source tree. There is no `catalyst = { path = ... }` or
//! `catalyst = { git = ... }` resolution. Forking-by-default.

use std::{
    fs,
    path::{Path, PathBuf},
};

use include_dir::{include_dir, Dir};

use crate::{
    codegen::build_rs_template,
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    project::{
        db_bootstrap::{self, BootstrapArgs, BootstrapOutcome, RealDbAdmin},
        post_install, preflight, templates,
    },
};

/// Framework source tree, baked into the blast binary at compile time.
/// `templates/canonical/` is the source of truth — edit there directly.
static CANONICAL: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/canonical");

pub struct Args {
    pub project_name: String,
    pub project_root: PathBuf,
    /// `.env` body to write. If `None`, falls back to the legacy default
    /// (so the existing scaffold tests that don't go through DB bootstrap
    /// keep working).
    pub env_body: Option<String>,
    /// `.env.test` body. If `None`, no `.env.test` file is written.
    pub env_test_body: Option<String>,
}

/// Optional callback signature for the post-seed pipeline. Receives the
/// fully-vendored project root, returns the count of additional files it
/// wrote (so the running tally in `Outcome` stays accurate). Lives behind
/// a function pointer so the lib-side scaffold module doesn't take a hard
/// dep on bin-private modules — gen_all, database, configs.
pub type PostSeedHook = dyn Fn(&Path, &mut dyn Sink, &mut dyn Progress) -> BlastResult<usize>;

#[derive(Default)]
pub struct NewOptions {
    pub db_url: Option<String>,
    pub force: bool,
    pub no_test_db: bool,
    pub no_warmup: bool,
    /// Optional callback invoked after `run()` lays files but before
    /// `post_install` exec's the dashboard. Used by the binary path to
    /// plug in the codegen pipeline + `cargo build` pre-compile. `None`
    /// from tests / lib consumers — no-op.
    pub post_seed: Option<std::sync::Arc<PostSeedHook>>,
}

pub struct Outcome {
    pub project_root: PathBuf,
    pub files_written: usize,
}

/// Scaffold a brand-new project at `<cwd>/<project_name>/`.
pub fn create_new_project_with_opts(project_name: &str, opts: NewOptions, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    let cwd = std::env::current_dir()?;
    let project_root = cwd.join(project_name);

    // Atomic ownership claim: create the leaf dir directly (was: exists-check
    // then create_dir_all later in run(); the gap was a TOCTOU window where
    // another process could create the dir between the two steps and our
    // create_dir_all would silently succeed, then we'd overwrite their content).
    // fs::create_dir errors with AlreadyExists if the path is taken — we treat
    // that as the same hard-fail the old `if exists() { Err }` produced.
    match project_root.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {} // allow: project_root has no parent (filesystem root); fs::create_dir below surfaces the right error
    }
    match fs::create_dir(&project_root) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(BlastError::Project(format!("directory `{}` already exists", project_root.display())));
        }
        Err(e) => return Err(BlastError::from(e)),
    }

    create_with_target(project_name, project_root, opts, sink, progress)
}

/// Scaffold IN PLACE at the given target dir. Used by `blast init`. The
/// dir must either be empty or `--force` must be set.
pub fn init_in_place_with_opts(project_name: &str, target_dir: PathBuf, opts: NewOptions, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    if target_dir.exists() {
        let is_empty = match fs::read_dir(&target_dir) {
            Ok(mut iter) => iter.next().is_none(),
            Err(e) => return Err(BlastError::Io(e)),
        };
        if !is_empty && !opts.force {
            return Err(BlastError::Project(format!("target directory `{}` is not empty (pass --force to overwrite)", target_dir.display())));
        }
    }
    create_with_target(project_name, target_dir, opts, sink, progress)
}

fn create_with_target(project_name: &str, project_root: PathBuf, opts: NewOptions, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    // The cargo package name must be the leaf directory name, not the full
    // path the user typed (`blast new /tmp/foo` → package `foo`, dir `/tmp/foo`).
    let package_name = match project_root.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_string(),
        None => {
            return Err(BlastError::Project(format!("could not derive a package name from `{}`", project_name)));
        }
    };

    sink.info(format!("scaffolding `{}` at {}", project_name, project_root.display()));
    sink.info("framework: vendored canonical (no `catalyst` dep)");

    // Preflight FIRST: verify required binaries are on PATH before we do
    // any disk writes or DB I/O. On any required-missing, this returns
    // an error immediately and the rest of the pipeline never runs.
    progress.step_start("preflight: required binaries");
    preflight::run(sink)?;
    progress.step_done("preflight: required binaries");

    // Bootstrap the database BEFORE writing any files. If this fails, the
    // user gets a clear error and no half-broken project lands on disk.
    progress.step_start("bootstrap database");
    let bootstrap = pre_create_db(&package_name, &opts, sink)?;
    progress.step_done("bootstrap database");

    let env_body = templates::env_example(&bootstrap.primary_url);
    let env_test_body = bootstrap.test_url.as_deref().map(templates::env_test_example);

    let args = Args {
        project_name: package_name,
        project_root: project_root.clone(),
        env_body: Some(env_body),
        env_test_body,
    };

    match run(args, sink, progress) {
        Ok(mut out) => {
            sink.success(format!("project `{}` created at {} ({} files written)", project_name, out.project_root.display(), out.files_written));

            // Phase 12: optional post-seed pipeline (codegen + cargo
            // pre-compile) injected from the binary layer. Lives outside
            // the lib because it depends on bin-private modules
            // (`commands`, `database`, `configs`). Skipped (no-op) when
            // the caller passes `None` — preserves the lib-only test
            // path that has no live DB.
            match opts.post_seed.as_deref() {
                Some(hook) => match hook(&out.project_root, sink, progress) {
                    Ok(extra_written) => {
                        out.files_written += extra_written;
                    }
                    Err(e) => {
                        sink.error(format!("post-seed pipeline failed: {}", e));
                        return Err(e);
                    }
                },
                None => {}
            }

            // Post-scaffold pipeline: warmup builds, exec into the
            // dashboard TUI. On the happy path, exec() replaces this
            // process and we never return from post_install::run.
            // If a warmup build fails the project dir is preserved
            // (so the user can inspect what broke) — the error is
            // surfaced but no cleanup runs.
            match post_install::run(&out.project_root, opts.no_warmup, sink, progress) {
                Ok(()) => {
                    // Reached only when BLAST_NO_TUI_FOR_TESTS=1 (skip
                    // the auto-TUI exec). Print the legacy next-steps
                    // hint so the user still has a manual path.
                    print_next_steps(project_name, sink);
                    Ok(out)
                }
                Err(e) => {
                    sink.error(format!("post-scaffold pipeline failed: {}", e));
                    Err(e)
                }
            }
        }
        Err(e) => {
            sink.error(format!("scaffolding failed: {}", e));
            if project_root.exists() {
                if let Err(cleanup_err) = fs::remove_dir_all(&project_root) {
                    sink.warn(format!("failed to clean up partial project dir {}: {}", project_root.display(), cleanup_err));
                }
            }
            Err(e)
        }
    }
}

/// Resolve the DB URL (CLI arg or interactive prompt), then run the DB
/// bootstrap orchestrator. Split out from `create_with_target` so the
/// file-writing `run()` path stays testable without a live Postgres.
fn pre_create_db(project_name: &str, opts: &NewOptions, sink: &mut dyn Sink) -> BlastResult<BootstrapOutcome> {
    let db_url = match &opts.db_url {
        Some(u) => u.clone(),
        None => resolve_db_url_default(project_name, sink)?,
    };

    let mut admin = RealDbAdmin;
    let bargs = BootstrapArgs {
        project_name: project_name.to_string(),
        db_url,
        force: opts.force,
        no_test_db: opts.no_test_db,
    };
    db_bootstrap::bootstrap(&bargs, &mut admin, sink)
}

/// Derive a sensible Postgres URL when the user didn't pass `--db-url`.
/// Tries the interactive prompt first (with the derived URL as the
/// pressable-Enter default); if the prompt fails (e.g. non-TTY stdin),
/// silently falls back to the derived default and surfaces it via the
/// sink so the user can override later via `.env`.
fn resolve_db_url_default(project_name: &str, sink: &mut dyn Sink) -> BlastResult<String> {
    let derived = db_bootstrap::default_url_for(project_name);
    sink.info(format!("no --db-url supplied; defaulting to `{}` (override later via .env)", derived));
    Ok(derived)
}

/// File-writing core. Receives fully-resolved Args (no DB I/O, no prompts).
/// Reusable from tests with a NullSink/NullProgress.
pub fn run(args: Args, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    let mut count: usize = 0;

    progress.step_start("create project root");
    fs::create_dir_all(&args.project_root)?;
    progress.step_done("create project root");

    progress.step_start("vendor canonical framework");
    let written = write_canonical(&args.project_root, &args.project_name)?;
    count += written;
    progress.step_done("vendor canonical framework");

    progress.step_start("write env files");
    write_env_files(&args, &mut count)?;
    progress.step_done("write env files");

    progress.step_start("emit build.rs hash check");
    let build_outcome = build_rs_template::run(build_rs_template::Args { project_root: args.project_root.clone() })?;
    sink.debug(format!("build.rs -> {}", build_outcome.written.display()));
    count += 1;
    progress.step_done("emit build.rs hash check");

    Ok(Outcome {
        project_root: args.project_root,
        files_written: count,
    })
}

/// Walk the embedded CANONICAL tree and write every file into
/// `project_root`, substituting `{{project_name}}` in both file paths and
/// file bodies. Returns the count of files written.
fn write_canonical(project_root: &Path, project_name: &str) -> BlastResult<usize> {
    let mut count = 0usize;
    write_dir_recursive(&CANONICAL, project_root, project_name, &mut count)?;
    Ok(count)
}

fn write_dir_recursive(dir: &Dir<'_>, project_root: &Path, project_name: &str, count: &mut usize) -> BlastResult<()> {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(d) => {
                let rel = d.path();
                if is_canonical_only_path(rel) {
                    continue;
                }
                let dest_path = project_root.join(substitute_path_component(rel, project_name));
                fs::create_dir_all(&dest_path)?;
                write_dir_recursive(d, project_root, project_name, count)?;
            }
            include_dir::DirEntry::File(f) => {
                let rel = f.path();
                if is_canonical_only_path(rel) {
                    continue;
                }
                let dest_path = project_root.join(substitute_path_component(rel, project_name));
                match dest_path.parent() {
                    Some(parent) => fs::create_dir_all(parent)?,
                    None => {} // allow: dest_path always has a parent (project_root.join(rel)), nothing to create
                }
                let body = render_file_body(f.contents(), project_name);
                fs::write(&dest_path, body)?;
                *count += 1;
            }
        }
    }
    Ok(())
}

/// Paths inside `templates/canonical/` that exist for canonical's own
/// in-place dev loop (target-dir redirect to dodge the include_dir blob
/// trap) but MUST NOT bleed into scaffolded user apps. Skipped wholesale
/// during vendor copy.
fn is_canonical_only_path(rel: &Path) -> bool {
    matches!(rel.to_string_lossy().as_ref(), ".cargo" | ".cargo/config.toml" | ".env" | ".env.test" | ".env.example")
}

fn substitute_path_component(rel: &Path, project_name: &str) -> PathBuf {
    let s = rel.to_string_lossy();
    if s.contains("{{project_name}}") {
        PathBuf::from(s.replace("{{project_name}}", project_name))
    } else {
        rel.to_path_buf()
    }
}

fn render_file_body(raw: &[u8], project_name: &str) -> Vec<u8> {
    match std::str::from_utf8(raw) {
        Ok(s) => {
            let mut out = s.to_string();
            if out.contains("{{project_name}}") {
                out = out.replace("{{project_name}}", project_name);
            }
            if out.contains("name = \"canonical\"") {
                out = out.replace("name = \"canonical\"", &format!("name = \"{}\"", project_name));
            }
            if out.contains("canonical::") {
                let crate_name = project_name.replace('-', "_");
                out = out.replace("canonical::", &format!("{crate_name}::"));
            }
            out.into_bytes()
        }
        Err(_not_utf8) => raw.to_vec(), // allow: binary asset, no substitution possible
    }
}

fn write_env_files(args: &Args, count: &mut usize) -> BlastResult<()> {
    let env_body = match &args.env_body {
        Some(body) => body.clone(),
        None => templates::env_example(&format!("postgres://postgres:postgres@localhost/{}", args.project_name)),
    };
    write_file(&args.project_root.join(".env.example"), &env_body, count)?;
    write_file(&args.project_root.join(".env"), &env_body, count)?;
    match &args.env_test_body {
        Some(test_body) => write_file(&args.project_root.join(".env.test"), test_body, count)?,
        None => {} // allow: no test DB requested, skip writing .env.test
    }
    Ok(())
}

fn write_file(target: &Path, body: &str, count: &mut usize) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("path has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(target, body)?;
    *count += 1;
    Ok(())
}

fn print_next_steps(project_name: &str, sink: &mut dyn Sink) {
    sink.info("next steps:");
    sink.info(format!("  cd {}", project_name));
    sink.info("  cargo run     # boot will run migrations + seed admin user");
    sink.info("  blast run     # (alternative) start dev server via blast");
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::io::null::{NullProgress, NullSink};

    /// Process-global cwd is shared across parallel `cargo test` threads.
    /// Any test that calls `env::set_current_dir` must serialize through
    /// this mutex; the local `CwdGuard` then restores cwd on Drop so a
    /// panic mid-test doesn't pollute subsequent tests.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    struct CwdGuard {
        prev: PathBuf,
        // Lock is held alongside the guard so the mutex releases AFTER cwd
        // is restored. Drop runs the impl body first, then drops fields in
        // declaration order — `prev: PathBuf` (no-op), then `_lock` releases.
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            match std::env::set_current_dir(&self.prev) {
                Ok(()) => {}
                Err(_restore_err) => {} // allow: best-effort cwd restore in Drop; can't propagate
            }
        }
    }

    fn enter_dir<P: AsRef<std::path::Path>>(target: P) -> CwdGuard {
        let lock = match CWD_LOCK.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let prev = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(target.as_ref()).expect("chdir");
        CwdGuard { prev, _lock: lock }
    }

    fn run_in_tempdir(name: &str) -> (tempfile::TempDir, Outcome) {
        let dir = tempfile::tempdir().expect("tempdir");
        let project_root = dir.path().join(name);
        let args = Args {
            project_name: name.to_string(),
            project_root,
            env_body: None,
            env_test_body: None,
        };
        let mut sink = NullSink;
        let mut progress = NullProgress;
        let outcome = run(args, &mut sink, &mut progress).expect("run");
        (dir, outcome)
    }

    #[test]
    fn run_writes_top_level_files() {
        let (_dir, outcome) = run_in_tempdir("acme");
        assert!(outcome.project_root.join("Cargo.toml").is_file());
        assert!(outcome.project_root.join(".gitignore").is_file());
        assert!(outcome.project_root.join(".env").is_file());
        assert!(outcome.project_root.join(".env.example").is_file());
        assert!(outcome.project_root.join("build.rs").is_file());
    }

    #[test]
    fn scaffold_writes_vendored_canonical() {
        let (_dir, outcome) = run_in_tempdir("acme");
        let root = &outcome.project_root;
        assert!(root.join("src").join("lib.rs").is_file(), "src/lib.rs not vendored");
        assert!(root.join("src").join("bootstrap.rs").is_file(), "src/bootstrap.rs not vendored");
        assert!(root.join("src").join("meltdown.rs").is_file(), "src/meltdown.rs not vendored");
        assert!(root.join("src").join("database").join("migrations").is_dir(), "src/database/migrations not vendored");
    }

    #[test]
    fn scaffold_substitutes_project_name_in_cargo_toml() {
        let (_dir, outcome) = run_in_tempdir("myapp");
        let body = fs::read_to_string(outcome.project_root.join("Cargo.toml")).expect("read Cargo.toml");
        assert!(body.contains(r#"name = "myapp""#), "expected name = \"myapp\" in Cargo.toml, got:\n{body}");
        assert!(!body.contains("{{project_name}}"), "Cargo.toml still has unsubstituted placeholder");
        assert!(!body.contains(r#"name = "catalyst""#), "Cargo.toml still labelled as catalyst");
    }

    #[test]
    fn scaffold_hyphen_package_name_underscores_in_rust_paths() {
        // FIX-034: cargo allows hyphens in package names but the crate identifier
        // replaces them with underscores. The `canonical::` → `<project_name>::`
        // substitution must use the underscored form to emit valid Rust.
        let (_dir, outcome) = run_in_tempdir("my-app");
        let cargo = fs::read_to_string(outcome.project_root.join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo.contains(r#"name = "my-app""#), "Cargo.toml must keep raw hyphenated name");

        let route_align = fs::read_to_string(outcome.project_root.join("tests").join("route_alignment_generated.rs")).expect("read route_alignment_generated.rs");
        assert!(
            route_align.contains("use my_app::structs::leptos::RouteName"),
            "expected `use my_app::...` (underscored crate path), got:\n{route_align}"
        );
        assert!(!route_align.contains("use my-app::"), "raw hyphen must NOT survive into Rust use statement");
        assert!(!route_align.contains("use canonical::"), "canonical:: substitution must have fired");
    }

    #[test]
    fn scaffold_no_hyphen_name_unchanged() {
        // Sanity: package names without hyphens pass through untouched
        // through the underscore conversion (idempotent on non-hyphen names).
        let (_dir, outcome) = run_in_tempdir("plainname");
        let route_align = fs::read_to_string(outcome.project_root.join("tests").join("route_alignment_generated.rs")).expect("read route_alignment_generated.rs");
        assert!(route_align.contains("use plainname::structs::leptos::RouteName"), "non-hyphen path should also substitute correctly");
    }

    #[test]
    fn scaffold_writes_env_files_via_args() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project_root = dir.path().join("acme");
        let args = Args {
            project_name: "acme".to_string(),
            project_root: project_root.clone(),
            env_body: Some("DATABASE_URL=postgres://x\n".to_string()),
            env_test_body: Some("DATABASE_URL=postgres://x_test\n".to_string()),
        };
        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(args, &mut sink, &mut progress).expect("run");
        let env = fs::read_to_string(project_root.join(".env")).expect("read .env");
        assert!(env.contains("postgres://x"));
        let env_test = fs::read_to_string(project_root.join(".env.test")).expect("read .env.test");
        assert!(env_test.contains("postgres://x_test"));
    }

    #[test]
    fn build_rs_marker_has_blake3_check() {
        let (_dir, outcome) = run_in_tempdir("acme");
        let body = fs::read_to_string(outcome.project_root.join("build.rs")).expect("read");
        assert!(body.contains("blake3"));
        assert!(body.contains("storage/blast/state/"));
    }

    #[test]
    fn scaffold_skips_canonical_only_cargo_config() {
        let (_dir, outcome) = run_in_tempdir("acme");
        let cargo_config = outcome.project_root.join(".cargo").join("config.toml");
        assert!(!cargo_config.exists(), "scaffold leaked canonical-only .cargo/config.toml into user app");
    }

    #[test]
    fn dashboard_kdl_is_vendored() {
        let (_dir, outcome) = run_in_tempdir("acme");
        let path = outcome.project_root.join("storage").join("blast").join("dashboard.kdl");
        assert!(path.is_file(), "dashboard.kdl not vendored at {}", path.display());
    }

    #[test]
    fn create_new_project_rejects_existing_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _cwd = enter_dir(dir.path());

        fs::create_dir_all(dir.path().join("dup")).expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        // We pass --no-test-db and an unreachable URL; the existing-dir check
        // fires BEFORE bootstrap, so the test still asserts the right thing
        // without needing a live Postgres.
        let opts = NewOptions {
            db_url: Some("postgres://nobody@127.0.0.1:1/x".to_string()),
            force: false,
            no_test_db: true,
            no_warmup: false,
            post_seed: None,
        };
        let result = create_new_project_with_opts("dup", opts, &mut sink, &mut progress);

        assert!(result.is_err());
        // _cwd drops here, restoring cwd + releasing CWD_LOCK.
    }

    #[test]
    fn create_new_project_does_not_overwrite_existing_dir_contents() {
        // Regression for the TOCTOU class: pre-fix used `if exists() { Err }`
        // then later `create_dir_all` (which silently succeeds on existing
        // dirs) then proceeded to write template files, overwriting any
        // collision. Post-fix uses atomic `fs::create_dir`, which errors
        // immediately on AlreadyExists — the user's prior content stays
        // intact even if the exists-check races with another process.
        let dir = tempfile::tempdir().expect("tempdir");
        let _cwd = enter_dir(dir.path());

        let target = dir.path().join("toctou_proj");
        fs::create_dir_all(&target).expect("seed");
        let canary = target.join("user_owned.txt");
        fs::write(&canary, "DO NOT TOUCH").expect("seed canary");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let opts = NewOptions {
            db_url: Some("postgres://nobody@127.0.0.1:1/x".to_string()),
            force: false,
            no_test_db: true,
            no_warmup: false,
            post_seed: None,
        };
        let result = create_new_project_with_opts("toctou_proj", opts, &mut sink, &mut progress);

        assert!(result.is_err(), "must fail-fast on existing dir");
        let canary_after = fs::read_to_string(&canary).expect("canary still readable");
        assert_eq!(canary_after, "DO NOT TOUCH", "scaffold leaked into the existing dir");
        // _cwd drops here, restoring cwd + releasing CWD_LOCK.
    }

    #[test]
    fn init_in_place_rejects_nonempty_dir_without_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("workspace");
        fs::create_dir_all(&target).expect("create");
        fs::write(target.join("preexisting.txt"), "x").expect("write");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let opts = NewOptions {
            db_url: Some("postgres://nobody@127.0.0.1:1/x".to_string()),
            force: false,
            no_test_db: true,
            no_warmup: false,
            post_seed: None,
        };
        let result = init_in_place_with_opts("workspace", target, opts, &mut sink, &mut progress);
        assert!(result.is_err());
    }
}
