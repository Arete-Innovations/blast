use crate::codegen::{build_rs_template, fe_runtime, frontend_scaffold};
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::project::{auth_migration, templates};
use crate::state::app::AppState;
use crate::state::io as state_io;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Args {
    pub project_name: String,
    pub project_root: PathBuf,
    pub catalyst_dep_line: String,
}

pub struct Outcome {
    pub project_root: PathBuf,
    pub files_written: usize,
    pub catalyst_dep_kind: CatalystDepKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalystDepKind {
    PathDep(PathBuf),
    GitDep,
}

pub fn create_new_project(
    project_name: &str,
    use_dev_branch: bool,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<Outcome> {
    let cwd = std::env::current_dir()?;
    let project_root = cwd.join(project_name);

    if project_root.exists() {
        return Err(BlastError::Project(format!(
            "directory `{}` already exists",
            project_root.display()
        )));
    }

    // The cargo package name must be the leaf directory name, not the full path
    // the user typed (`blast new /tmp/foo` → package `foo`, dir `/tmp/foo`).
    let package_name = match project_root.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_string(),
        None => {
            return Err(BlastError::Project(format!(
                "could not derive a package name from `{}`",
                project_name
            )));
        }
    };

    let (catalyst_dep_line, dep_kind) = match resolve_catalyst_dep(use_dev_branch, &project_root) {
        Some((line, kind)) => (line, kind),
        None => (templates::catalyst_git_dep(), CatalystDepKind::GitDep),
    };

    sink.info(format!(
        "scaffolding `{}` at {}",
        project_name,
        project_root.display()
    ));
    match &dep_kind {
        CatalystDepKind::PathDep(path) => {
            sink.info(format!("catalyst dep resolved to local path: {}", path.display()));
        }
        CatalystDepKind::GitDep => {
            sink.info("catalyst dep set to git (no local checkout found)");
        }
    }

    let args = Args {
        project_name: package_name,
        project_root: project_root.clone(),
        catalyst_dep_line,
    };

    let outcome = run(args, sink, progress);
    match outcome {
        Ok(mut out) => {
            out.catalyst_dep_kind = dep_kind;
            sink.success(format!(
                "project `{}` created at {} ({} files written)",
                project_name,
                out.project_root.display(),
                out.files_written
            ));
            print_next_steps(project_name, sink);
            Ok(out)
        }
        Err(e) => {
            sink.error(format!("scaffolding failed: {}", e));
            if project_root.exists() {
                if let Err(cleanup_err) = fs::remove_dir_all(&project_root) {
                    sink.warn(format!(
                        "failed to clean up partial project dir {}: {}",
                        project_root.display(),
                        cleanup_err
                    ));
                }
            }
            Err(e)
        }
    }
}

pub fn run(
    args: Args,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<Outcome> {
    let mut count: usize = 0;

    progress.step_start("create project root");
    fs::create_dir_all(&args.project_root)?;
    progress.step_done("create project root");

    progress.step_start("write Cargo.toml");
    write_file(
        &args.project_root.join("Cargo.toml"),
        &templates::cargo_toml(&args.project_name, &args.catalyst_dep_line),
        &mut count,
    )?;
    progress.step_done("write Cargo.toml");

    progress.step_start("write top-level files");
    write_file(
        &args.project_root.join(".gitignore"),
        templates::gitignore(),
        &mut count,
    )?;
    let env_body = templates::env_example(&args.project_name);
    write_file(
        &args.project_root.join(".env.example"),
        &env_body,
        &mut count,
    )?;
    write_file(&args.project_root.join(".env"), &env_body, &mut count)?;
    progress.step_done("write top-level files");

    progress.step_start("scaffold src/ layer tree");
    scaffold_src_tree(&args.project_root, &mut count)?;
    progress.step_done("scaffold src/ layer tree");

    progress.step_start("scaffold storage state");
    scaffold_storage_state(&args.project_root, &mut count)?;
    progress.step_done("scaffold storage state");

    progress.step_start("scaffold migrations");
    scaffold_migrations(&args.project_root, &mut count)?;
    progress.step_done("scaffold migrations");

    progress.step_start("emit build.rs");
    let build_outcome = build_rs_template::run(build_rs_template::Args {
        project_root: args.project_root.clone(),
    })?;
    sink.debug(format!("build.rs -> {}", build_outcome.written.display()));
    count += 1;
    progress.step_done("emit build.rs");

    progress.step_start("scaffold frontend");
    scaffold_frontend(&args.project_root, &args.project_name, &mut count)?;
    progress.step_done("scaffold frontend");

    progress.step_start("seed frontend tokens/base/primevue");
    let scaffold_outcome = frontend_scaffold::run(&args.project_root)?;
    for path in &scaffold_outcome.written {
        sink.debug(format!("seeded {}", path.display()));
        count += 1;
    }
    progress.step_done("seed frontend tokens/base/primevue");

    progress.step_start("seed frontend runtime (page shell, router, progress)");
    let runtime_outcome = fe_runtime::run(&args.project_root, &args.project_name)?;
    for path in &runtime_outcome.written {
        sink.debug(format!("seeded {}", path.display()));
        count += 1;
    }
    progress.step_done("seed frontend runtime (page shell, router, progress)");

    Ok(Outcome {
        project_root: args.project_root,
        files_written: count,
        catalyst_dep_kind: CatalystDepKind::GitDep,
    })
}

fn write_file(target: &Path, body: &str, count: &mut usize) -> BlastResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| BlastError::Invalid(format!("path has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(target, body)?;
    *count += 1;
    Ok(())
}

fn scaffold_src_tree(project_root: &Path, count: &mut usize) -> BlastResult<()> {
    let src_root = project_root.join("src");
    fs::create_dir_all(&src_root)?;

    write_file(&src_root.join("main.rs"), templates::main_rs(), count)?;

    for layer in templates::LAYERS_WITH_GENERATED_AND_CUSTOM {
        let paths = templates::layer_paths(&src_root, layer);
        write_file(&paths.mod_path, templates::layer_mod_rs(), count)?;
        write_file(
            &paths.generated_mod,
            templates::empty_layer_inner_mod_rs(),
            count,
        )?;
        write_file(
            &paths.custom_mod,
            templates::empty_custom_mod_rs(),
            count,
        )?;
    }

    let transport_dir = src_root.join("transport");
    fs::create_dir_all(&transport_dir)?;
    write_file(
        &transport_dir.join("mod.rs"),
        templates::transport_mod_rs(),
        count,
    )?;
    for sub in templates::TRANSPORT_SUBLAYERS {
        let paths = templates::transport_sublayer_paths(&src_root, sub);
        write_file(&paths.mod_path, templates::layer_mod_rs(), count)?;
        write_file(
            &paths.generated_mod,
            templates::empty_layer_inner_mod_rs(),
            count,
        )?;
        write_file(
            &paths.custom_mod,
            templates::empty_custom_mod_rs(),
            count,
        )?;
    }

    let database_dir = src_root.join("database");
    fs::create_dir_all(&database_dir)?;
    write_file(
        &database_dir.join("mod.rs"),
        templates::database_mod_rs(),
        count,
    )?;
    write_file(
        &database_dir.join("schema.rs"),
        templates::database_schema_rs(),
        count,
    )?;

    Ok(())
}

fn scaffold_storage_state(project_root: &Path, count: &mut usize) -> BlastResult<()> {
    let state_dir = project_root.join("storage").join("blast").join("state");
    fs::create_dir_all(&state_dir)?;

    let app_state = AppState::new();
    state_io::save_app(&state_dir, &app_state)?;
    *count += 1;

    let resources_dir = state_dir.join("resources");
    fs::create_dir_all(&resources_dir)?;
    write_file(
        &resources_dir.join(".gitkeep"),
        templates::gitkeep_marker(),
        count,
    )?;

    let blast_dir = project_root.join("storage").join("blast");
    write_file(
        &blast_dir.join(".gitignore"),
        templates::storage_blast_gitignore(),
        count,
    )?;
    write_file(
        &blast_dir.join("dashboard.kdl"),
        templates::dashboard_kdl(),
        count,
    )?;

    let logs_dir = project_root.join("storage").join("logs");
    fs::create_dir_all(&logs_dir)?;
    write_file(
        &logs_dir.join(".gitkeep"),
        templates::gitkeep_marker(),
        count,
    )?;

    Ok(())
}

fn scaffold_migrations(project_root: &Path, count: &mut usize) -> BlastResult<()> {
    let migrations_dir = project_root.join("migrations");
    fs::create_dir_all(&migrations_dir)?;
    let written = auth_migration::emit(&migrations_dir)?;
    *count += written.len();
    Ok(())
}

fn scaffold_frontend(
    project_root: &Path,
    project_name: &str,
    count: &mut usize,
) -> BlastResult<()> {
    let frontend_root = project_root.join("frontend");
    fs::create_dir_all(&frontend_root)?;

    write_file(
        &frontend_root.join("package.json"),
        &templates::frontend_package_json(project_name),
        count,
    )?;
    // index.html and src/main.ts are owned by codegen::fe_runtime; the
    // runtime-scaffold step seeds them after this fn returns.
    write_file(
        &frontend_root.join("vite.config.ts"),
        templates::frontend_vite_config_ts(),
        count,
    )?;
    write_file(
        &frontend_root.join("tsconfig.json"),
        templates::frontend_tsconfig_json(),
        count,
    )?;

    let fe_src = frontend_root.join("src");
    fs::create_dir_all(&fe_src)?;
    write_file(
        &fe_src.join("App.vue"),
        templates::frontend_app_vue(),
        count,
    )?;

    let fe_generated = fe_src.join("generated");
    fs::create_dir_all(&fe_generated)?;
    write_file(
        &fe_generated.join(".gitkeep"),
        templates::gitkeep_marker(),
        count,
    )?;

    let fe_custom = fe_src.join("custom");
    fs::create_dir_all(&fe_custom)?;
    write_file(
        &fe_custom.join(".gitkeep"),
        templates::gitkeep_marker(),
        count,
    )?;

    Ok(())
}

fn resolve_catalyst_dep(
    _use_dev_branch: bool,
    project_root: &Path,
) -> Option<(String, CatalystDepKind)> {
    // Always prefer a sibling catalyst checkout when one exists. Git fallback
    // kicks in only when no local catalyst is reachable. The legacy
    // `_use_dev_branch` flag is retained for CLI compatibility but no longer
    // gates the search — there's no scenario where you want to ignore a
    // local catalyst and pull from git instead.
    let candidate = find_sibling_catalyst()?;
    let relative = path_relative_from(project_root, &candidate)?;
    let line = templates::catalyst_path_dep(&relative.to_string_lossy());
    Some((line, CatalystDepKind::PathDep(candidate)))
}

fn find_sibling_catalyst() -> Option<PathBuf> {
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(_e) => return None,
    };
    let mut dir: Option<&Path> = Some(cwd.as_path());
    loop {
        let current = match dir {
            Some(c) => c,
            None => return None,
        };
        let candidate = current.join("catalyst");
        if candidate.is_dir() && candidate.join("Cargo.toml").is_file() {
            return Some(candidate);
        }
        dir = current.parent();
    }
}

fn path_relative_from(from_dir: &Path, target: &Path) -> Option<PathBuf> {
    let from_components: Vec<_> = from_dir.components().collect();
    let target_components: Vec<_> = target.components().collect();

    let mut common = 0usize;
    while common < from_components.len()
        && common < target_components.len()
        && from_components[common] == target_components[common]
    {
        common += 1;
    }

    let ups = from_components.len().saturating_sub(common);
    let mut out = PathBuf::new();
    for _ in 0..ups {
        out.push("..");
    }
    for c in &target_components[common..] {
        out.push(c.as_os_str());
    }
    if out.as_os_str().is_empty() {
        return None;
    }
    Some(out)
}

fn print_next_steps(project_name: &str, sink: &mut dyn Sink) {
    sink.info("next steps:");
    sink.info(format!("  cd {}", project_name));
    sink.info("  blast init    # run migrations + codegen pipeline");
    sink.info("  blast run     # start dev server");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::{NullProgress, NullSink};

    fn run_in_tempdir(name: &str, dep_line: &str) -> (tempfile::TempDir, Outcome) {
        let dir = tempfile::tempdir().expect("tempdir");
        let project_root = dir.path().join(name);
        let args = Args {
            project_name: name.to_string(),
            project_root,
            catalyst_dep_line: dep_line.to_string(),
        };
        let mut sink = NullSink;
        let mut progress = NullProgress;
        let outcome = run(args, &mut sink, &mut progress).expect("run");
        (dir, outcome)
    }

    #[test]
    fn run_creates_top_level_files() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        assert!(outcome.project_root.join("Cargo.toml").is_file());
        assert!(outcome.project_root.join(".gitignore").is_file());
        assert!(outcome.project_root.join(".env").is_file());
        assert!(outcome.project_root.join(".env.example").is_file());
        assert!(outcome.project_root.join("build.rs").is_file());
    }

    #[test]
    fn run_creates_layer_dirs_with_generated_and_custom() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let src = outcome.project_root.join("src");
        for layer in templates::LAYERS_WITH_GENERATED_AND_CUSTOM {
            assert!(
                src.join(layer).join("mod.rs").is_file(),
                "missing mod.rs for {}",
                layer
            );
            assert!(
                src.join(layer).join("generated").join("mod.rs").is_file(),
                "missing generated/mod.rs for {}",
                layer
            );
            assert!(
                src.join(layer).join("custom").join("mod.rs").is_file(),
                "missing custom/mod.rs for {}",
                layer
            );
        }
    }

    #[test]
    fn run_creates_transport_sublayers() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let transport = outcome.project_root.join("src").join("transport");
        assert!(transport.join("mod.rs").is_file());
        for sub in templates::TRANSPORT_SUBLAYERS {
            assert!(transport.join(sub).join("mod.rs").is_file());
            assert!(transport
                .join(sub)
                .join("generated")
                .join("mod.rs")
                .is_file());
            assert!(transport.join(sub).join("custom").join("mod.rs").is_file());
        }
    }

    #[test]
    fn run_creates_database_skeleton() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let database = outcome.project_root.join("src").join("database");
        assert!(database.join("mod.rs").is_file());
        assert!(database.join("schema.rs").is_file());
    }

    #[test]
    fn run_creates_storage_state_with_app_ron() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let state = outcome
            .project_root
            .join("storage")
            .join("blast")
            .join("state");
        assert!(state.join("app.ron").is_file());
        assert!(state.join("resources").join(".gitkeep").is_file());
        let app_body = fs::read_to_string(state.join("app.ron")).expect("read app.ron");
        assert!(app_body.contains("schema_version"));
    }

    #[test]
    fn run_creates_migrations_dir() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let up_sql = outcome
            .project_root
            .join("migrations")
            .join("0001_users_and_sessions")
            .join("up.sql");
        assert!(up_sql.is_file(), "0001_users_and_sessions/up.sql not found");
        let body = fs::read_to_string(&up_sql).expect("read up.sql");
        assert!(body.contains("CREATE TABLE users"), "up.sql missing CREATE TABLE users");
    }

    #[test]
    fn run_creates_migration_down_sql() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let down_sql = outcome
            .project_root
            .join("migrations")
            .join("0001_users_and_sessions")
            .join("down.sql");
        assert!(down_sql.is_file(), "0001_users_and_sessions/down.sql not found");
    }

    #[test]
    fn run_creates_frontend_skeleton() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let fe = outcome.project_root.join("frontend");
        assert!(fe.join("package.json").is_file());
        assert!(fe.join("index.html").is_file());
        assert!(fe.join("vite.config.ts").is_file());
        assert!(fe.join("tsconfig.json").is_file());
        assert!(fe.join("src").join("main.ts").is_file());
        assert!(fe.join("src").join("App.vue").is_file());
        assert!(fe.join("src").join("generated").join(".gitkeep").is_file());
        assert!(fe.join("src").join("custom").join(".gitkeep").is_file());
        assert!(fe.join("src").join("styles").join("tokens.css").is_file());
        assert!(fe.join("src").join("styles").join("base.css").is_file());
        assert!(fe
            .join("src")
            .join("plugins")
            .join("primevue.ts")
            .is_file());
    }

    #[test]
    fn cargo_toml_has_catalyst_dep_line() {
        let (_dir, outcome) =
            run_in_tempdir("acme", r#"catalyst = { path = "../foo", features = ["testing"] }"#);
        let body = fs::read_to_string(outcome.project_root.join("Cargo.toml")).expect("read");
        assert!(body.contains(r#"catalyst = { path = "../foo""#));
        assert!(body.contains(r#"name = "acme""#));
    }

    #[test]
    fn build_rs_marker_has_blake3_check() {
        let (_dir, outcome) = run_in_tempdir("acme", &templates::catalyst_git_dep());
        let body = fs::read_to_string(outcome.project_root.join("build.rs")).expect("read");
        assert!(body.contains("blake3"));
        assert!(body.contains("storage/blast/state/"));
    }

    #[test]
    fn create_new_project_rejects_existing_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let original = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(dir.path()).expect("chdir");

        fs::create_dir_all(dir.path().join("dup")).expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let result = create_new_project("dup", false, &mut sink, &mut progress);

        std::env::set_current_dir(original).expect("restore cwd");
        assert!(result.is_err());
    }

    #[test]
    fn path_relative_from_walks_up() {
        let from = Path::new("/a/b/c/project");
        let to = Path::new("/a/b/catalyst");
        let out = path_relative_from(from, to).expect("relative");
        assert_eq!(out, PathBuf::from("../../catalyst"));
    }

    #[test]
    fn path_relative_from_handles_sibling() {
        let from = Path::new("/work/myapp");
        let to = Path::new("/work/catalyst");
        let out = path_relative_from(from, to).expect("relative");
        assert_eq!(out, PathBuf::from("../catalyst"));
    }
}
