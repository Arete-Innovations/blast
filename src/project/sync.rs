use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::project::scaffold::Source;
use crate::state::{self, app::AppPolicySection};

/// Project-relative path of the app state file consulted for the freeze list.
const APP_STATE_REL: &str = "storage/blast/state";

const VENDORED_PATHS: &[&str] = &[
    "src/views/components/vendored",
    "src/views/builders/vendored",
    "src/views/signals/vendored",
    "src/structs/vendored",
    "src/transport/leptos/vendored",
    "src/transport/http/vendored",
    "src/transport/ws/vendored",
    "src/transport/fuses/vendored",
    "src/models/vendored",
    "src/services/vendored",
    "src/meltdown.rs",
    "src/crank.rs",
    "src/cata_log.rs",
    "src/ctx.rs",
    "build.rs",
    "style",
];

pub fn run_sync(dev: bool, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<()> {
    let project_root = std::env::current_dir()?;
    if !project_root.join("Cargo.toml").is_file() {
        return Err(BlastError::Project(format!("no Cargo.toml in {} — run from project root", project_root.display())));
    }

    let source = match dev {
        true => Source::dev_from_env()?,
        false => Source::git_default(),
    };

    if let Source::LocalCopy { path, .. } = &source {
        if path == &project_root {
            return Err(BlastError::Project(format!(
                "blast sync --dev: BLAST_CATALYST_DEV_PATH ({}) is the project root — refusing (would truncate files via self-overwrite)",
                path.display()
            )));
        }
    }

    let temp = tempfile::tempdir()?;
    let staging: std::path::PathBuf = match &source {
        Source::LocalCopy { path, .. } => {
            sink.info(format!("blast sync: reading working tree at {} (dev mode — no git clone)", path.display()));
            path.clone()
        }
        Source::Git { url, .. } => {
            let target = temp.path().join("catalyst");
            sink.info(format!("blast sync: cloning catalyst from {} into tempdir", url));
            progress.step_start("clone catalyst");
            clone_catalyst(&source, &target, sink)?;
            progress.step_done("clone catalyst");
            target
        }
    };

    let freeze = load_freeze_list(&project_root, sink);
    if !freeze.is_empty() {
        sink.info(format!("freeze list active: {} entr(ies) — frozen paths will be skipped", freeze.len()));
    }

    progress.step_start("rsync vendored paths");
    let mut copied = 0usize;
    let mut frozen_skips = 0usize;
    for rel in VENDORED_PATHS {
        let src = staging.join(rel);
        if !src.exists() {
            sink.debug(format!("skip {} (not in catalyst)", rel));
            continue;
        }
        if is_frozen(rel, &freeze) {
            sink.info(format!("frozen: skipped {}", rel));
            frozen_skips += 1;
            continue;
        }
        let dst = project_root.join(rel);
        let skipped = copy_recursive(&src, &dst, &project_root, &freeze, sink)?;
        frozen_skips += skipped;
        copied += 1;
        sink.info(format!("synced {}", rel));
    }
    progress.step_done("rsync vendored paths");
    if frozen_skips > 0 {
        sink.info(format!("frozen: {} path(s) skipped", frozen_skips));
    }

    progress.step_start("merge Cargo.toml deps");
    let cargo_upstream = staging.join("Cargo.toml");
    let cargo_project = project_root.join("Cargo.toml");
    if cargo_upstream.is_file() && cargo_project.is_file() {
        let up = std::fs::read_to_string(&cargo_upstream)?;
        let pj = std::fs::read_to_string(&cargo_project)?;
        match merge_cargo_toml(&up, &pj) {
            Ok(merged) if merged != pj => {
                std::fs::write(&cargo_project, &merged)?;
                sink.info("merged Cargo.toml deps from catalyst".to_string());
            }
            Ok(_unchanged) => {
                sink.debug("Cargo.toml deps already in sync".to_string());
            }
            Err(e) => sink.warn(format!("Cargo.toml merge skipped: {}", e)),
        }
    }
    progress.step_done("merge Cargo.toml deps");

    sink.success(format!("blast sync: {} path(s) synced from catalyst", copied));
    Ok(())
}

fn clone_catalyst(source: &Source, target: &Path, sink: &mut dyn Sink) -> BlastResult<()> {
    let url = match source {
        Source::Git { url, .. } => url.clone(),
        Source::LocalCopy { path, .. } => path.to_string_lossy().into_owned(),
    };
    let branch = match source {
        Source::Git { branch, .. } => branch.as_str(),
        Source::LocalCopy { branch, .. } => branch.as_str(),
    };

    sink.debug(format!("git clone --branch {} {} {}", branch, url, target.display()));

    let status = Command::new("git")
        .args(["clone", "--branch", branch, "--no-hardlinks", "--single-branch", "--depth", "1"])
        .arg(&url)
        .arg(target)
        .status()
        .map_err(|e| BlastError::Project(format!("failed to spawn `git clone`: {}", e)))?;

    match status.success() {
        true => Ok(()),
        false => Err(BlastError::Project(format!(
            "git clone failed (exit {}): branch {} from {}",
            status.code().unwrap_or(-1), // allow: signal-killed exit has no code; -1 sentinel
            branch,
            url
        ))),
    }
}

fn copy_recursive(src: &Path, dst: &Path, project_root: &Path, freeze: &[String], sink: &mut dyn Sink) -> BlastResult<usize> {
    if src.is_file() {
        let relpath = relpath_for(dst, project_root);
        if is_frozen(&relpath, freeze) {
            sink.info(format!("frozen: skipped {}", relpath));
            return Ok(1);
        }
        match dst.parent() {
            Some(parent) => std::fs::create_dir_all(parent)?,
            None => {} // allow: dst is filesystem root, no parent to create
        }
        std::fs::copy(src, dst)?;
        return Ok(0);
    }
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        return copy_dir_contents(src, dst, project_root, freeze, sink);
    }
    sink.warn(format!("source path is neither file nor dir: {}", src.display()));
    Ok(0)
}

fn merge_cargo_toml(upstream: &str, project: &str) -> BlastResult<String> {
    use toml_edit::{Array, DocumentMut, Item, Table};

    let mut pj_doc: DocumentMut = project.parse().map_err(|e| BlastError::Project(format!("project Cargo.toml parse: {}", e)))?;
    let up_doc: DocumentMut = upstream.parse().map_err(|e| BlastError::Project(format!("upstream Cargo.toml parse: {}", e)))?;

    fn merge_dep_table(pj: &mut Table, up: &Table) {
        for (key, up_val) in up.iter() {
            match pj.get_mut(key) {
                None => {
                    pj.insert(key, up_val.clone());
                }
                Some(pj_val) => {
                    let up_inline = up_val.as_inline_table();
                    let pj_inline = pj_val.as_inline_table_mut();
                    match (pj_inline, up_inline) {
                        (Some(pj_tbl), Some(up_tbl)) => {
                            for (sub_k, sub_v) in up_tbl.iter() {
                                let existing = pj_tbl.get(sub_k);
                                let up_arr = sub_v.as_array();
                                let is_features = sub_k == "features";
                                let merged_features: Option<Array> = match (is_features, existing.and_then(|e| e.as_array()), up_arr) {
                                    (true, Some(ex_arr), Some(up_arr_v)) => {
                                        let mut union: Array = ex_arr.clone();
                                        let have: std::collections::HashSet<String> =
                                            ex_arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                                        for v in up_arr_v.iter() {
                                            match v.as_str() {
                                                Some(s) => match have.contains(s) {
                                                    true => {} // allow: dup feature, skip
                                                    false => union.push(s),
                                                },
                                                None => {} // allow: non-string feature entry skipped
                                            }
                                        }
                                        Some(union)
                                    }
                                    (_, _, _) => None,
                                };
                                match merged_features {
                                    Some(arr) => {
                                        pj_tbl.insert(sub_k, toml_edit::Value::Array(arr));
                                    }
                                    None => {
                                        pj_tbl.insert(sub_k, sub_v.clone());
                                    }
                                }
                            }
                        }
                        (_, _) => {
                            *pj_val = up_val.clone();
                        }
                    }
                }
            }
        }
    }

    fn merge_section(pj_doc: &mut DocumentMut, up_doc: &DocumentMut, name: &str) {
        let up_item = match up_doc.get(name) {
            Some(Item::Table(t)) => t.clone(),
            _ => return, // allow: section absent in upstream
        };
        let pj_item = pj_doc.entry(name).or_insert(Item::Table(Table::new()));
        match pj_item.as_table_mut() {
            Some(pj_tbl) => merge_dep_table(pj_tbl, &up_item),
            None => {} // allow: project entry exists but is not a table; leave as-is
        }
    }

    for name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        merge_section(&mut pj_doc, &up_doc, name);
    }

    let target_keys: Vec<String> = match up_doc.get("target").and_then(|i| i.as_table()) {
        Some(t) => t.iter().map(|(k, _)| k.to_string()).collect(),
        None => Vec::new(), // allow: upstream has no [target.*]
    };
    for tk in target_keys {
        for sub in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let up_sub = match up_doc.get("target").and_then(|t| t.get(&tk)).and_then(|t| t.get(sub)).and_then(|i| i.as_table()) {
                Some(t) => t.clone(),
                None => continue,
            };
            let pj_target = pj_doc.entry("target").or_insert(Item::Table(Table::new()));
            let pj_target_tbl = match pj_target.as_table_mut() {
                Some(t) => t,
                None => continue, // allow: malformed [target] in project, leave alone
            };
            let pj_tk = pj_target_tbl.entry(&tk).or_insert(Item::Table(Table::new()));
            let pj_tk_tbl = match pj_tk.as_table_mut() {
                Some(t) => t,
                None => continue, // allow: malformed [target.X] in project
            };
            let pj_sub = pj_tk_tbl.entry(sub).or_insert(Item::Table(Table::new()));
            match pj_sub.as_table_mut() {
                Some(pj_sub_tbl) => merge_dep_table(pj_sub_tbl, &up_sub),
                None => continue, // allow: malformed [target.X.deps] in project
            }
        }
    }

    Ok(pj_doc.to_string())
}

fn merge_barrel(upstream: &str, project: &str) -> String {
    let project_keys: HashSet<String> = project.lines().filter_map(decl_key).collect();
    let upstream_by_key: HashMap<String, String> = upstream.lines().filter_map(|l| decl_key(l).map(|k| (k, l.to_string()))).collect();

    let mut out = String::new();
    for line in project.lines() {
        match decl_key(line) {
            Some(key) => match upstream_by_key.get(&key) {
                Some(upstream_line) => {
                    out.push_str(upstream_line);
                    out.push('\n');
                }
                None => {
                    out.push_str(line);
                    out.push('\n');
                }
            },
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }

    let mut new_lines: Vec<&str> = Vec::new();
    for line in upstream.lines() {
        match decl_key(line) {
            Some(key) => match project_keys.contains(&key) {
                true => {} // allow: project already has this decl, walk-loop swapped in upstream version
                false => new_lines.push(line),
            },
            None => {} // allow: non-decl upstream lines (comments/blanks) come through walk-loop only
        }
    }
    if !new_lines.is_empty() {
        if !out.ends_with("\n\n") && out.ends_with('\n') {
            out.push('\n');
        }
        for line in new_lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn decl_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let mod_split = match trimmed.strip_prefix("pub mod ") {
        Some(rest) => Some(("pub mod ", rest)),
        None => trimmed.strip_prefix("mod ").map(|rest| ("mod ", rest)),
    };
    match mod_split {
        Some((kw, rest)) => {
            let head = rest.split(|c: char| c == ';' || c == '{').next()?;
            return Some(format!("{}{}", kw, head.trim()));
        }
        None => {} // allow: not a mod decl, fall through to use-decl branch
    }
    let (kw, after) = match trimmed.strip_prefix("pub use ") {
        Some(rest) => ("pub use ", rest),
        None => match trimmed.strip_prefix("use ") {
            Some(rest) => ("use ", rest),
            None => return None,
        },
    };
    let stop_brace = after.find("::{");
    let stop_semi = after.find(';');
    let cut = match (stop_brace, stop_semi) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    Some(format!("{}{}", kw, after[..cut].trim()))
}

#[cfg(test)]
mod tests {
    use super::{merge_barrel, merge_cargo_toml};

    #[test]
    fn cargo_merge_appends_new_wasm_target_dep() {
        let upstream = r#"[package]
name = "catalyst"

[dependencies]
serde = "1"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen-futures = "0.4"
"#;
        let project = r#"[package]
name = "tweetbook"

[dependencies]
serde = "1"

[target.'cfg(target_arch = "wasm32")'.dependencies]
"#;
        let merged = merge_cargo_toml(upstream, project).expect("merge");
        assert!(merged.contains("wasm-bindgen-futures"), "expected wasm-bindgen-futures, got:\n{merged}");
        assert!(merged.contains(r#"name = "tweetbook""#), "expected project name preserved");
    }

    #[test]
    fn cargo_merge_unions_features_array() {
        let upstream = r#"[target.'cfg(target_arch = "wasm32")'.dependencies]
web-sys = { version = "0.3", features = ["Window", "Navigator", "Clipboard"] }
"#;
        let project = r#"[target.'cfg(target_arch = "wasm32")'.dependencies]
web-sys = { version = "0.3", features = ["Window", "Storage"] }
"#;
        let merged = merge_cargo_toml(upstream, project).expect("merge");
        assert!(merged.contains("Storage"), "project-only feature kept");
        assert!(merged.contains("Navigator"), "upstream feature appended");
        assert!(merged.contains("Clipboard"), "upstream feature appended");
    }

    #[test]
    fn cargo_merge_preserves_project_only_dep() {
        let upstream = r#"[dependencies]
serde = "1"
"#;
        let project = r#"[dependencies]
serde = "1"
my-app-only-dep = "0.1"
"#;
        let merged = merge_cargo_toml(upstream, project).expect("merge");
        assert!(merged.contains("my-app-only-dep"), "project-only dep preserved");
    }

    #[test]
    fn appends_new_pub_mod_from_upstream() {
        let upstream = "pub mod a;\npub mod b;\n";
        let project = "pub mod a;\n";
        let merged = merge_barrel(upstream, project);
        assert!(merged.contains("pub mod a;"));
        assert!(merged.contains("pub mod b;"));
    }

    #[test]
    fn upstream_pub_use_rename_supersedes_project() {
        let upstream = "pub use foo::{a, b, c};\n";
        let project = "pub use foo::{a, b};\n";
        let merged = merge_barrel(upstream, project);
        assert_eq!(merged, "pub use foo::{a, b, c};\n");
    }

    #[test]
    fn keeps_app_only_lines() {
        let upstream = "pub mod a;\n";
        let project = "pub mod a;\npub mod app_added;\n";
        let merged = merge_barrel(upstream, project);
        assert!(merged.contains("pub mod a;"));
        assert!(merged.contains("pub mod app_added;"));
    }

    #[test]
    fn no_change_when_identical() {
        let same = "pub mod a;\npub use a::*;\n";
        let merged = merge_barrel(same, same);
        assert_eq!(merged, same);
    }
}

fn copy_dir_contents(src: &Path, dst: &Path, project_root: &Path, freeze: &[String], sink: &mut dyn Sink) -> BlastResult<usize> {
    let mut frozen_skips = 0usize;
    for entry_res in std::fs::read_dir(src)? {
        let entry = entry_res?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type()?;
        match kind.is_dir() {
            true => {
                let rel = relpath_for(&to, project_root);
                if is_frozen(&rel, freeze) {
                    sink.info(format!("frozen: skipped {}/", rel));
                    frozen_skips += 1;
                    continue;
                }
                std::fs::create_dir_all(&to)?;
                frozen_skips += copy_dir_contents(&from, &to, project_root, freeze, sink)?;
            }
            false => {
                let rel = relpath_for(&to, project_root);
                if is_frozen(&rel, freeze) {
                    sink.info(format!("frozen: skipped {}", rel));
                    frozen_skips += 1;
                    continue;
                }
                let name = entry.file_name();
                let is_barrel = name.to_string_lossy() == "mod.rs";
                if is_barrel && to.exists() {
                    let upstream = std::fs::read_to_string(&from)?;
                    let project = std::fs::read_to_string(&to)?;
                    let merged = merge_barrel(&upstream, &project);
                    if merged != project {
                        std::fs::write(&to, merged)?;
                    }
                } else {
                    std::fs::copy(&from, &to)?;
                }
            }
        }
    }
    Ok(frozen_skips)
}

/// Project-root-relative path of `abs`, normalised to forward slashes.
/// Falls back to the absolute path on strip-prefix failure (defensive —
/// the caller always passes paths inside `project_root`).
fn relpath_for(abs: &Path, project_root: &Path) -> String {
    let stripped = match abs.strip_prefix(project_root) {
        Ok(p) => p.to_path_buf(),
        Err(_) => abs.to_path_buf(),
    };
    stripped.to_string_lossy().replace('\\', "/")
}

/// True iff `relpath` is covered by some freeze entry. Exact match wins;
/// directory entries (non-trailing-slash) match `relpath == entry` and
/// `relpath` starting with `entry/`. Empty entries are ignored.
fn is_frozen(relpath: &str, freeze: &[String]) -> bool {
    freeze
        .iter()
        .filter(|e| !e.is_empty())
        .any(|entry| relpath == entry.as_str() || relpath.starts_with(&format!("{entry}/")))
}

/// Read the project's `app.ron` freeze list. Soft-fails (returns empty
/// list + warns) if the state file is missing or unreadable — sync still
/// runs at full vendored overwrite, matching pre-freeze behavior.
fn load_freeze_list(project_root: &Path, sink: &mut dyn Sink) -> Vec<String> {
    let state_dir = project_root.join(APP_STATE_REL);
    if !state_dir.is_dir() {
        return Vec::new();
    }
    match state::load_app(&state_dir) {
        Ok(app) => app.freeze_list(),
        Err(e) => {
            sink.warn(format!("freeze list ignored: failed to load app.ron ({e})"));
            Vec::new()
        }
    }
}

pub fn run_diff(dev: bool, only: Option<&str>, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<()> {
    let project_root = std::env::current_dir()?;
    if !project_root.join("Cargo.toml").is_file() {
        return Err(BlastError::Project(format!("no Cargo.toml in {} — run from project root", project_root.display())));
    }

    let freeze = load_freeze_list(&project_root, sink);
    let targets: Vec<String> = match only {
        Some(p) => match freeze.iter().any(|f| f == p) {
            true => vec![p.to_string()],
            false => return Err(BlastError::Project(format!("'{p}' is not in the freeze list"))),
        },
        None => freeze.clone(),
    };
    if targets.is_empty() {
        sink.info("freeze list is empty — nothing to diff".to_string());
        return Ok(());
    }

    let staging = stage_catalyst(dev, &project_root, sink, progress)?;

    for entry in &targets {
        let local = project_root.join(entry);
        let upstream = staging.join(entry);
        if !upstream.exists() {
            sink.warn(format!("'{entry}' not present in catalyst — nothing to diff against"));
            continue;
        }
        if !local.exists() {
            sink.warn(format!("'{entry}' not present locally — would be a fresh add on unfreeze"));
            continue;
        }
        sink.info(format!("=== diff: {entry} (left=local, right=catalyst) ==="));
        run_diff_pair(&local, &upstream)?;
    }

    Ok(())
}

fn stage_catalyst(dev: bool, project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<PathBuf> {
    let source = match dev {
        true => Source::dev_from_env()?,
        false => Source::git_default(),
    };
    if let Source::LocalCopy { path, .. } = &source {
        if path == project_root {
            return Err(BlastError::Project(format!(
                "blast sync diff --dev: BLAST_CATALYST_DEV_PATH ({}) is the project root — refusing self-diff",
                path.display()
            )));
        }
    }
    let target: PathBuf = match &source {
        Source::LocalCopy { path, .. } => {
            sink.info(format!("blast sync diff: reading working tree at {} (dev mode)", path.display()));
            path.clone()
        }
        Source::Git { url, .. } => {
            // Persist clone for the duration of the call — diff invokes external `diff`
            // process which reads paths after this fn returns.
            let temp = tempfile::tempdir()?.keep();
            let tgt = temp.join("catalyst");
            sink.info(format!("blast sync diff: cloning catalyst from {} into tempdir", url));
            progress.step_start("clone catalyst");
            clone_catalyst(&source, &tgt, sink)?;
            progress.step_done("clone catalyst");
            tgt
        }
    };
    Ok(target)
}

fn run_diff_pair(local: &Path, upstream: &Path) -> BlastResult<()> {
    if local.is_dir() && upstream.is_dir() {
        let status = Command::new("diff")
            .args(["-ruN", "--", &local.to_string_lossy(), &upstream.to_string_lossy()])
            .status()
            .map_err(|e| BlastError::Project(format!("failed to spawn `diff`: {e}")))?;
        // diff exit code: 0=same, 1=different, ≥2=trouble. 0 and 1 are both "ran fine".
        match status.code() {
            Some(0) | Some(1) => Ok(()),
            other => Err(BlastError::Project(format!("`diff` failed (exit {:?})", other))),
        }
    } else {
        let status = Command::new("diff")
            .args(["-u", "--", &local.to_string_lossy(), &upstream.to_string_lossy()])
            .status()
            .map_err(|e| BlastError::Project(format!("failed to spawn `diff`: {e}")))?;
        match status.code() {
            Some(0) | Some(1) => Ok(()),
            other => Err(BlastError::Project(format!("`diff` failed (exit {:?})", other))),
        }
    }
}

pub fn run_unfreeze(path: &str, sink: &mut dyn Sink) -> BlastResult<()> {
    let project_root = std::env::current_dir()?;
    if !project_root.join("Cargo.toml").is_file() {
        return Err(BlastError::Project(format!("no Cargo.toml in {} — run from project root", project_root.display())));
    }
    let state_dir = project_root.join(APP_STATE_REL);
    let mut app = state::load_app(&state_dir)?;

    let removed = match app.sections.get_mut("sync") {
        Some(AppPolicySection::Sync(cfg)) => {
            let before = cfg.freeze.len();
            cfg.freeze.retain(|e| e != path);
            before != cfg.freeze.len()
        }
        _ => false,
    };

    match removed {
        true => {
            state::save_app(&state_dir, &app)?;
            sink.success(format!("unfroze '{path}' — next sync will overwrite it"));
        }
        false => sink.warn(format!("'{path}' was not in the freeze list — nothing changed")),
    }
    Ok(())
}

