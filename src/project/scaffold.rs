//! Project scaffolder.
//!
//! `blast new <name>` and `blast init [<name>]` both land here. The
//! framework lives in a separate repo (catalyst). Scaffolding is a
//! `git clone` + a small Cargo.toml substitution. No bundled tree.
//!
//! Source resolution:
//! - Default: clone `https://github.com/ZmoleCristian/catalyst` master
//! - `--dev`: use `BLAST_CATALYST_DEV_PATH` env var as the source path
//!   (still goes through `git clone <local-path>` so the user's project
//!   inherits catalyst's history; no special-case rsync flow).
//!
//! Substitution: 3 lines in Cargo.toml only.
//! - `[package].name = "catalyst"` → `<project_name>`
//! - `[[bin]].name   = "catalyst"` → `<project_name>`
//! - `[package.metadata.leptos] output-name = "catalyst"` → `<project_name>`
//!
//! `[lib].name = "catalyst"` STAYS — anchors `tests/*.rs use catalyst::*`
//! across all forks. `git pull upstream master` from a spawned project
//! only conflicts on those 3 Cargo.toml lines; src/ and tests/ merge
//! cleanly forever.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    codegen::build_rs_template,
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    project::{
        db_bootstrap::{self, BootstrapArgs, BootstrapOutcome, RealDbAdmin},
        post_install, preflight, templates,
    },
};

const CATALYST_GIT_URL: &str = "https://github.com/ZmoleCristian/catalyst";
const CATALYST_BRANCH: &str = "master";
const DEV_PATH_ENV: &str = "BLAST_CATALYST_DEV_PATH";

/// Where the framework source comes from. Both variants resolve through
/// `git clone` — a URL or a local path are equivalent to git.
pub enum Source {
    /// Clone from a git URL.
    Git { url: String, branch: String },
    /// Clone from a local catalyst checkout (still via `git clone <path>`).
    LocalCopy { path: PathBuf, branch: String },
}

impl Source {
    pub fn git_default() -> Self {
        Self::Git {
            url: CATALYST_GIT_URL.to_string(),
            branch: CATALYST_BRANCH.to_string(),
        }
    }

    /// Resolve `--dev` mode: read `BLAST_CATALYST_DEV_PATH` env var.
    /// Returns Err if unset/empty — there's no auto-discovery; we don't
    /// guess where catalyst lives on the user's disk.
    pub fn dev_from_env() -> BlastResult<Self> {
        match std::env::var(DEV_PATH_ENV) {
            Ok(p) if !p.is_empty() => Ok(Self::LocalCopy {
                path: PathBuf::from(p),
                branch: CATALYST_BRANCH.to_string(),
            }),
            _ => Err(BlastError::Project(format!(
                "--dev mode requires {} env var to point at a local catalyst dir",
                DEV_PATH_ENV
            ))),
        }
    }

    fn url_for_clone(&self) -> String {
        match self {
            Self::Git { url, .. } => url.clone(),
            Self::LocalCopy { path, .. } => path.to_string_lossy().into_owned(),
        }
    }

    fn branch(&self) -> &str {
        match self {
            Self::Git { branch, .. } => branch,
            Self::LocalCopy { branch, .. } => branch,
        }
    }
}

pub struct Args {
    pub project_name: String,
    pub project_root: PathBuf,
    pub source: Source,
    /// `.env` body to write. If `None`, falls back to a default derived
    /// from project_name (used by tests that don't run DB bootstrap).
    pub env_body: Option<String>,
    /// `.env.test` body. If `None`, no `.env.test` file is written.
    pub env_test_body: Option<String>,
}

pub type PostSeedHook = dyn Fn(&Path, &mut dyn Sink, &mut dyn Progress) -> BlastResult<usize>;

#[derive(Default)]
pub struct NewOptions {
    pub db_url: Option<String>,
    pub force: bool,
    pub no_test_db: bool,
    pub no_warmup: bool,
    /// Use local catalyst from `BLAST_CATALYST_DEV_PATH` instead of git URL.
    pub dev: bool,
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

    // Atomic ownership claim: create the leaf dir directly. fs::create_dir
    // errors with AlreadyExists if the path is taken.
    //
    // Wrinkle: `git clone` refuses to clone into an existing dir (even an
    // empty one). So we DON'T create the dir — we just check the parent
    // exists and the leaf doesn't, then let git create it.
    match project_root.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {} // allow: project_root has no parent; git clone surfaces the right error
    }
    if project_root.exists() {
        return Err(BlastError::Project(format!("directory `{}` already exists", project_root.display())));
    }

    create_with_target(project_name, project_root, opts, sink, progress)
}

/// Scaffold IN PLACE at the given target dir. Used by `blast init`. The
/// dir must either be empty or `--force` must be set.
///
/// Wrinkle: `git clone` refuses to clone into an existing non-empty dir.
/// For init mode we workaround by cloning to a sibling tempdir and moving
/// contents over.
pub fn init_in_place_with_opts(project_name: &str, target_dir: PathBuf, opts: NewOptions, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    if target_dir.exists() {
        let is_empty = match fs::read_dir(&target_dir) {
            Ok(mut iter) => iter.next().is_none(),
            Err(e) => return Err(BlastError::Io(e)),
        };
        if !is_empty && !opts.force {
            return Err(BlastError::Project(format!("target directory `{}` is not empty (pass --force to overwrite)", target_dir.display())));
        }
    } else {
        fs::create_dir_all(&target_dir)?;
    }
    create_with_target(project_name, target_dir, opts, sink, progress)
}

fn create_with_target(project_name: &str, project_root: PathBuf, opts: NewOptions, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    let package_name = match project_root.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_string(),
        None => return Err(BlastError::Project(format!("could not derive a package name from `{}`", project_name))),
    };

    let source = if opts.dev { Source::dev_from_env()? } else { Source::git_default() };

    sink.info(format!("scaffolding `{}` at {}", project_name, project_root.display()));
    match &source {
        Source::Git { url, branch } => sink.info(format!("framework: git clone {} (branch {})", url, branch)),
        Source::LocalCopy { path, branch } => sink.info(format!("framework: git clone {} (branch {})", path.display(), branch)),
    }

    progress.step_start("preflight: required binaries");
    preflight::run(sink)?;
    progress.step_done("preflight: required binaries");

    progress.step_start("bootstrap database");
    let bootstrap = pre_create_db(&package_name, &opts, sink)?;
    progress.step_done("bootstrap database");

    let env_body = templates::env_example(&bootstrap.primary_url);
    let env_test_body = bootstrap.test_url.as_deref().map(templates::env_test_example);

    let args = Args {
        project_name: package_name,
        project_root: project_root.clone(),
        source,
        env_body: Some(env_body),
        env_test_body,
    };

    match run(args, sink, progress) {
        Ok(mut out) => {
            sink.success(format!("project `{}` created at {} ({} files written)", project_name, out.project_root.display(), out.files_written));

            match opts.post_seed.as_deref() {
                Some(hook) => match hook(&out.project_root, sink, progress) {
                    Ok(extra_written) => out.files_written += extra_written,
                    Err(e) => {
                        sink.error(format!("post-seed pipeline failed: {}", e));
                        return Err(e);
                    }
                },
                None => {} // allow: no post-seed hook (lib/test path); skip silently
            }

            match post_install::run(&out.project_root, opts.no_warmup, sink, progress) {
                Ok(()) => {
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

fn resolve_db_url_default(project_name: &str, sink: &mut dyn Sink) -> BlastResult<String> {
    let derived = db_bootstrap::default_url_for(project_name);
    sink.info(format!("no --db-url supplied; defaulting to `{}` (override later via .env)", derived));
    Ok(derived)
}

/// File-writing core. Receives fully-resolved Args.
pub fn run(args: Args, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    let mut count: usize = 0;

    progress.step_start("clone framework");
    clone_framework(&args.source, &args.project_root, sink)?;
    let cloned_files = count_tracked_files(&args.project_root)?;
    count += cloned_files;
    progress.step_done("clone framework");

    progress.step_start("rename git remote origin -> upstream");
    rename_origin_to_upstream(&args.project_root, sink)?;
    progress.step_done("rename git remote origin -> upstream");

    progress.step_start("substitute project name in Cargo.toml");
    apply_cargo_substitutions(&args.project_root, &args.project_name)?;
    progress.step_done("substitute project name in Cargo.toml");

    progress.step_start("write env files");
    write_env_files(&args, &mut count)?;
    progress.step_done("write env files");

    progress.step_start("emit build.rs hash check");
    let build_outcome = build_rs_template::run(build_rs_template::Args { project_root: args.project_root.clone() })?;
    sink.debug(format!("build.rs -> {}", build_outcome.written.display()));
    count += 1;
    progress.step_done("emit build.rs hash check");

    progress.step_start("seed empty stylance index");
    seed_stylance_placeholder(&args.project_root)?;
    count += 1;
    progress.step_done("seed empty stylance index");

    Ok(Outcome {
        project_root: args.project_root,
        files_written: count,
    })
}

/// `style/main.scss` does `@use "generated/stylance"` which Dart Sass resolves
/// at `cargo leptos build` time — BEFORE `cargo build` runs catalyst's `build.rs`
/// (which calls `stylance` to populate the file). Without a placeholder, the
/// first leptos build fails on missing stylesheet. We seed an empty file so
/// Sass succeeds; build.rs overwrites it with real hashed CSS on first compile.
fn seed_stylance_placeholder(project_root: &Path) -> BlastResult<()> {
    let dir = project_root.join("style").join("generated");
    fs::create_dir_all(&dir)?;
    let bundle = dir.join("stylance.scss");
    if !bundle.exists() {
        fs::write(&bundle, "// placeholder — overwritten by stylance-cli on first cargo build\n")?;
    }
    Ok(())
}

fn clone_framework(source: &Source, project_root: &Path, sink: &mut dyn Sink) -> BlastResult<()> {
    let url = source.url_for_clone();
    let branch = source.branch();

    sink.debug(format!("git clone --branch {} --no-hardlinks {} {}", branch, url, project_root.display()));

    let status = Command::new("git")
        .args(["clone", "--branch", branch, "--no-hardlinks", "--single-branch"])
        .arg(&url)
        .arg(project_root)
        .status()
        .map_err(|e| BlastError::Project(format!("failed to spawn `git clone`: {}", e)))?;

    if !status.success() {
        return Err(BlastError::Project(format!(
            "git clone failed (exit {}): branch {} from {}",
            status.code().unwrap_or(-1),
            branch,
            url
        )));
    }
    Ok(())
}

fn rename_origin_to_upstream(project_root: &Path, sink: &mut dyn Sink) -> BlastResult<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["remote", "rename", "origin", "upstream"])
        .status()
        .map_err(|e| BlastError::Project(format!("failed to spawn `git remote rename`: {}", e)))?;

    if !status.success() {
        // Non-fatal: if the user used `--dev` against a path with no
        // origin remote, the rename fails harmlessly. Log + continue.
        sink.warn("git remote rename origin -> upstream did not succeed (non-fatal — no origin remote?)".to_string());
    }
    Ok(())
}

fn count_tracked_files(project_root: &Path) -> BlastResult<usize> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["ls-files"])
        .output()
        .map_err(|e| BlastError::Project(format!("failed to spawn `git ls-files`: {}", e)))?;

    if !output.status.success() {
        return Ok(0);
    }
    Ok(output.stdout.split(|b| *b == b'\n').filter(|line| !line.is_empty()).count())
}

/// Section-aware Cargo.toml rewrite. Touches `name = "catalyst"` only
/// inside `[package]` and `[[bin]]`, and `output-name = "catalyst"` inside
/// `[package.metadata.leptos]`. Critically, `[lib].name = "catalyst"` is
/// LEFT UNTOUCHED — it anchors test imports across all forks.
fn apply_cargo_substitutions(project_root: &Path, project_name: &str) -> BlastResult<()> {
    let cargo_path = project_root.join("Cargo.toml");
    let body = fs::read_to_string(&cargo_path)?;

    let mut out = String::with_capacity(body.len());
    let mut current_section = String::new();

    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            // Section header. Use rfind so `[[bin]]` (array-of-tables) is
            // captured as `[[bin]]`, not truncated to `[[bin]` by the first
            // closing bracket.
            let end = trimmed.rfind(']').map(|i| i + 1).unwrap_or(trimmed.len());
            current_section = trimmed[..end].to_string();
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let new_line = match current_section.as_str() {
            "[package]" | "[[bin]]" => replace_name_line(line, project_name),
            "[package.metadata.leptos]" => replace_output_name_line(line, project_name),
            _ => None,
        };

        match new_line {
            Some(replaced) => out.push_str(&replaced),
            None => out.push_str(line),
        }
        out.push('\n');
    }

    // body.lines() drops the trailing newline if any. Restore one.
    if !out.ends_with('\n') {
        out.push('\n');
    }

    fs::write(&cargo_path, out)?;
    Ok(())
}

fn replace_name_line(line: &str, project_name: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed == r#"name = "catalyst""# || trimmed == r#"name="catalyst""# {
        Some(format!(r#"name = "{}""#, project_name))
    } else {
        None
    }
}

fn replace_output_name_line(line: &str, project_name: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed == r#"output-name = "catalyst""# || trimmed == r#"output-name="catalyst""# {
        Some(format!(r#"output-name = "{}""#, project_name))
    } else {
        None
    }
}

fn write_env_files(args: &Args, count: &mut usize) -> BlastResult<()> {
    let env_body = match &args.env_body {
        Some(body) => body.clone(),
        None => templates::env_example(&format!("postgres://postgres:postgres@localhost/{}", args.project_name)),
    };
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
    sink.info("framework updates:");
    sink.info("  git pull upstream master    # merge new framework commits (3-line Cargo.toml conflict expected)");
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::io::null::{NullProgress, NullSink};

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    struct CwdGuard {
        prev: PathBuf,
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

    /// Build a minimal fake catalyst repo in tempdir so `git clone <path>`
    /// works without network access. Three sections + a master commit.
    fn fixture_catalyst(dir: &Path) -> PathBuf {
        let repo = dir.join("fake-catalyst");
        fs::create_dir_all(&repo).expect("mkdir repo");
        fs::write(
            repo.join("Cargo.toml"),
            r#"[package]
name = "catalyst"
version = "0.1.0"
edition = "2021"

[lib]
name = "catalyst"
crate-type = ["cdylib", "rlib"]

[[bin]]
name = "catalyst"
path = "src/main.rs"

[package.metadata.leptos]
output-name = "catalyst"
"#,
        )
        .expect("Cargo.toml");
        fs::create_dir_all(repo.join("src")).expect("src");
        fs::write(repo.join("src/main.rs"), "fn main() {}\n").expect("main.rs");
        fs::write(repo.join("src/lib.rs"), "// catalyst lib\n").expect("lib.rs");
        fs::create_dir_all(repo.join("tests")).expect("tests");
        fs::write(repo.join("tests/smoke.rs"), "use catalyst::*;\n").expect("test");

        // Initialize git, set author, branch=master, commit.
        let g = |args: &[&str]| {
            Command::new("git").arg("-C").arg(&repo).args(args).status().expect("git").success()
        };
        assert!(g(&["init", "-b", "master"]));
        assert!(g(&["config", "user.email", "test@test"]));
        assert!(g(&["config", "user.name", "test"]));
        assert!(g(&["add", "."]));
        assert!(g(&["commit", "-m", "init"]));

        repo
    }

    fn run_in_tempdir(name: &str) -> (tempfile::TempDir, Outcome) {
        let dir = tempfile::tempdir().expect("tempdir");
        let fixture = fixture_catalyst(dir.path());
        let project_root = dir.path().join(name);
        let args = Args {
            project_name: name.to_string(),
            project_root,
            source: Source::LocalCopy {
                path: fixture,
                branch: "master".to_string(),
            },
            env_body: None,
            env_test_body: None,
        };
        let mut sink = NullSink;
        let mut progress = NullProgress;
        let outcome = run(args, &mut sink, &mut progress).expect("run");
        (dir, outcome)
    }

    #[test]
    fn run_clones_framework_into_project_root() {
        let (_dir, outcome) = run_in_tempdir("acme");
        assert!(outcome.project_root.join("Cargo.toml").is_file());
        assert!(outcome.project_root.join("src").join("main.rs").is_file());
        assert!(outcome.project_root.join(".env").is_file());
        assert!(outcome.project_root.join("build.rs").is_file());
        assert!(outcome.project_root.join(".git").is_dir(), "git history must be retained");
    }

    #[test]
    fn cargo_toml_substitutes_package_bin_output_name_only() {
        let (_dir, outcome) = run_in_tempdir("myapp");
        let body = fs::read_to_string(outcome.project_root.join("Cargo.toml")).expect("read Cargo.toml");

        // [package].name -> myapp (split by next section header [lib])
        let pkg_section = body.split("[package]").nth(1).and_then(|s| s.split("\n[").next()).expect("[package] section present");
        assert!(
            pkg_section.contains(r#"name = "myapp""#),
            "[package].name MUST become myapp — got [package]{pkg_section}"
        );

        // [[bin]].name -> myapp (regression: double-bracket array-of-tables
        // was previously truncated to `[[bin]` by `find(']')` — caught by e2e
        // smoke against catalyst, fixed by rfind).
        let bin_section = body.split("[[bin]]").nth(1).and_then(|s| s.split("\n[").next()).expect("[[bin]] section present");
        assert!(
            bin_section.contains(r#"name = "myapp""#),
            "[[bin]].name MUST become myapp — got [[bin]]{bin_section}"
        );

        // [package.metadata.leptos] output-name -> myapp
        assert!(body.contains(r#"output-name = "myapp""#), "expected output-name = \"myapp\", got:\n{body}");

        // [lib].name STAYS catalyst — anchors test imports
        let lib_section = body.split("[lib]").nth(1).and_then(|s| s.split("\n[").next()).expect("[lib] section present");
        assert!(
            lib_section.contains(r#"name = "catalyst""#),
            "[lib].name MUST stay catalyst (anchors tests/*.rs use catalyst::*) — got [lib]{lib_section}"
        );
    }

    #[test]
    fn cargo_toml_no_canonical_substitution_in_source() {
        // tests/smoke.rs uses `use catalyst::*` and we should NOT touch it.
        // The whole point of [lib].name = "catalyst" anchor is so source/test
        // code is package-name-agnostic.
        let (_dir, outcome) = run_in_tempdir("anything");
        let test_body = fs::read_to_string(outcome.project_root.join("tests").join("smoke.rs")).expect("read test");
        assert_eq!(test_body, "use catalyst::*;\n", "test files must be byte-identical with catalyst");
    }

    #[test]
    fn origin_renamed_to_upstream() {
        let (_dir, outcome) = run_in_tempdir("acme");
        let remotes = Command::new("git")
            .arg("-C")
            .arg(&outcome.project_root)
            .args(["remote"])
            .output()
            .expect("git remote");
        let s = String::from_utf8_lossy(&remotes.stdout);
        assert!(s.contains("upstream"), "expected `upstream` remote, got: {s}");
        assert!(!s.contains("origin"), "origin should have been renamed away");
    }

    #[test]
    fn env_files_written_via_args() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fixture = fixture_catalyst(dir.path());
        let project_root = dir.path().join("acme");
        let args = Args {
            project_name: "acme".to_string(),
            project_root: project_root.clone(),
            source: Source::LocalCopy {
                path: fixture,
                branch: "master".to_string(),
            },
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
    fn build_rs_has_no_stale_detection() {
        let (_dir, outcome) = run_in_tempdir("acme");
        let body = fs::read_to_string(outcome.project_root.join("build.rs")).expect("read");
        assert!(!body.contains("blake3"), "stale-detection killed");
        assert!(!body.contains("AUTO-GENERATED"), "no marker parsing");
        assert!(body.contains("check_transport_handler_ctx"), "TRANSPORT:23 still wired");
    }

    #[test]
    fn create_new_project_rejects_existing_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _cwd = enter_dir(dir.path());

        fs::create_dir_all(dir.path().join("dup")).expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let opts = NewOptions {
            db_url: Some("postgres://nobody@127.0.0.1:1/x".to_string()),
            force: false,
            no_test_db: true,
            no_warmup: false,
            dev: false,
            post_seed: None,
        };
        let result = create_new_project_with_opts("dup", opts, &mut sink, &mut progress);

        assert!(result.is_err());
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
            dev: false,
            post_seed: None,
        };
        let result = init_in_place_with_opts("workspace", target, opts, &mut sink, &mut progress);
        assert!(result.is_err());
    }

    #[test]
    fn dev_mode_requires_env_var() {
        // Save current value if any, clear it for the test.
        let prev = std::env::var(DEV_PATH_ENV).ok();
        std::env::remove_var(DEV_PATH_ENV);
        let result = Source::dev_from_env();
        // Restore env var.
        if let Some(v) = prev {
            std::env::set_var(DEV_PATH_ENV, v);
        }
        assert!(result.is_err(), "dev mode without {} should error", DEV_PATH_ENV);
    }
}
