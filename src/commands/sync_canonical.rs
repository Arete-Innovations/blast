//! `blast sync-canonical [--catalyst-path <path>] [--check]`
//!
//! Refresh the vendored Catalyst snapshot under `blast/templates/canonical/`
//! from a live catalyst checkout. This mutates the BLAST source tree itself
//! — used by Catablast maintainers when catalyst evolves and the vendored
//! snapshot needs to be re-baked into the next blast binary build.
//!
//! Not a runtime end-user command. End users get whatever snapshot was
//! baked into their installed blast binary.
//!
//! Excludes: `target/`, `worktrees/`, `.git/`, `Cargo.lock`,
//! `node_modules/`, `frontend/dist/`, plus internal noise like
//! `CLAUDE.md`, `.claude/`, `check_errors.sh`, `.env`, log files, server.pid.
//!
//! Auto-edits the copied `Cargo.toml`:
//!   - drops the `[workspace]` block (vendored apps aren't workspace anchors)
//!   - swaps `name = "catalyst"` -> `name = "{{project_name}}"`
//!
//! Adds `.gitkeep` markers to empty `storage/` subdirs so `include_dir!`
//! preserves their structure.
//!
//! PATH-AWARE SYNC: Only paths in `CANONICAL_SOURCE_PATHS` are mirrored from
//! catalyst. Paths in `templates/canonical/` that are NOT in this list are
//! blast-owned and are never touched by sync (neither overwritten nor deleted).
//! Currently the only blast-owned path is `frontend/` (FE shell vendored by
//! Blast, never sourced from catalyst per Wave 9 lock).

use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Sink, SinkExt};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Paths under catalyst that sync-canonical mirrors into templates/canonical/.
/// Anything in templates/canonical/ NOT in this list is blast-owned and
/// preserved across syncs. Currently the only such path is `frontend/` (FE
/// shell vendored by Blast, never sourced from catalyst per Wave 9 lock).
///
/// Trailing slashes on directory entries are cosmetic only; matching uses
/// `is_under_source_path` which checks the first path component.
const CANONICAL_SOURCE_PATHS: &[&str] = &[
    "src/",
    "doc/",
    "Cargo.toml",
    "diesel.toml",
    "rustfmt.toml",
    "LICENSE",
    "public/",
    "storage/",
];

#[derive(Debug, Clone)]
pub struct Args {
    /// Path to the live catalyst checkout to copy from. If `None`, defaults
    /// to a sibling `../catalyst/` resolved from the blast crate root.
    pub catalyst_path: Option<PathBuf>,
    /// Where to write the snapshot. Defaults to `<blast-root>/templates/canonical/`.
    pub destination: PathBuf,
    /// If true, diff without writing; non-zero exit if drift is detected.
    pub check: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub files_copied: usize,
    pub files_skipped: usize,
    pub drift_paths: Vec<PathBuf>,
    pub destination: PathBuf,
}

pub fn run(args: Args, sink: &mut dyn Sink) -> BlastResult<Outcome> {
    let source = match args.catalyst_path.clone() {
        Some(p) => p,
        None => default_catalyst_path()?,
    };

    if !source.is_dir() {
        return Err(BlastError::Invalid(format!(
            "catalyst source path `{}` is not a directory",
            source.display()
        )));
    }
    if !source.join("Cargo.toml").is_file() {
        return Err(BlastError::Invalid(format!(
            "`{}` does not look like a catalyst checkout (no Cargo.toml)",
            source.display()
        )));
    }

    sink.info(format!(
        "syncing canonical from {} -> {}",
        source.display(),
        args.destination.display()
    ));

    if args.check {
        return run_check(&source, &args.destination, sink);
    }

    // Path-aware sync: only mirror catalyst-owned paths. Blast-owned paths
    // (e.g. `frontend/`) in templates/canonical/ are left untouched.
    //
    // Strategy: for each entry in CANONICAL_SOURCE_PATHS, wipe the
    // corresponding subtree in the destination (so catalyst deletions
    // propagate), then walk the source subtree and copy files into dest.
    fs::create_dir_all(&args.destination)?;

    let mut files_copied = 0usize;
    let mut files_skipped = 0usize;

    for &source_path in CANONICAL_SOURCE_PATHS {
        let canonical_name = source_path.trim_end_matches('/');
        let src_entry = source.join(canonical_name);
        let dst_entry = args.destination.join(canonical_name);

        // Wipe the catalyst-owned subtree in dest so removals propagate.
        if dst_entry.exists() {
            if dst_entry.is_dir() {
                fs::remove_dir_all(&dst_entry)?;
            } else {
                fs::remove_file(&dst_entry)?;
            }
        }

        // If source doesn't have this path at all, nothing to copy.
        if !src_entry.exists() {
            continue;
        }

        // Single file (e.g. Cargo.toml, diesel.toml, LICENSE).
        if src_entry.is_file() {
            let rel = Path::new(canonical_name);
            if should_skip(rel) {
                files_skipped += 1;
                continue;
            }
            let body = fs::read(&src_entry)?;
            let body = patch_body_if_cargo_toml(rel, body);
            fs::write(&dst_entry, body)?;
            files_copied += 1;
            continue;
        }

        // Directory subtree.
        for entry in WalkDir::new(&src_entry) {
            let entry = match entry {
                Ok(e) => e,
                Err(walk_err) => {
                    return Err(BlastError::Invalid(format!(
                        "walkdir failed under `{}`: {}",
                        src_entry.display(),
                        walk_err
                    )));
                }
            };
            let path = entry.path();
            let rel = match path.strip_prefix(&source) {
                Ok(r) => r,
                Err(strip_err) => {
                    return Err(BlastError::Invalid(format!(
                        "strip_prefix failed for `{}`: {}",
                        path.display(),
                        strip_err
                    )));
                }
            };
            if should_skip(rel) {
                files_skipped += 1;
                continue;
            }
            let dest = args.destination.join(rel);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&dest)?;
                continue;
            }
            if entry.file_type().is_file() {
                match dest.parent() {
                    Some(parent) => fs::create_dir_all(parent)?,
                    None => {} // allow: always has a parent
                }
                let body = fs::read(path)?;
                let body = patch_body_if_cargo_toml(rel, body);
                fs::write(&dest, body)?;
                files_copied += 1;
            }
        }
    }

    // Plant .gitkeep markers in known-empty storage subdirs so include_dir
    // preserves them.
    plant_gitkeeps(&args.destination)?;

    sink.success(format!(
        "synced {} files (skipped {})",
        files_copied, files_skipped
    ));

    Ok(Outcome {
        files_copied,
        files_skipped,
        drift_paths: Vec::new(),
        destination: args.destination,
    })
}

fn run_check(source: &Path, destination: &Path, sink: &mut dyn Sink) -> BlastResult<Outcome> {
    let mut drift: Vec<PathBuf> = Vec::new();
    let mut files_checked = 0usize;
    let mut files_skipped = 0usize;

    // Path-aware check: only compare catalyst-owned paths.
    // Blast-owned paths (e.g. `frontend/`) are never counted as drift.
    for &source_path in CANONICAL_SOURCE_PATHS {
        let canonical_name = source_path.trim_end_matches('/');
        let src_entry = source.join(canonical_name);

        // If catalyst doesn't have this path, skip — nothing to check.
        if !src_entry.exists() {
            continue;
        }

        // Determine the walk root: single file or directory subtree.
        let walk_root = if src_entry.is_file() {
            // Treat as a single-item walk by using the parent dir and
            // filtering — simpler to just handle inline.
            let rel = Path::new(canonical_name);
            if should_skip(rel) {
                files_skipped += 1;
                continue;
            }
            files_checked += 1;
            let dest = destination.join(rel);
            if !dest.is_file() {
                drift.push(rel.to_path_buf());
            } else {
                let src_body = fs::read(&src_entry)?;
                let expected = patch_body_if_cargo_toml(rel, src_body);
                let actual = fs::read(&dest)?;
                if expected != actual {
                    drift.push(rel.to_path_buf());
                }
            }
            continue;
        } else {
            src_entry.clone()
        };

        for entry in WalkDir::new(&walk_root) {
            let entry = match entry {
                Ok(e) => e,
                Err(walk_err) => {
                    return Err(BlastError::Invalid(format!(
                        "walkdir failed under `{}`: {}",
                        walk_root.display(),
                        walk_err
                    )));
                }
            };
            let path = entry.path();
            let rel = match path.strip_prefix(source) {
                Ok(r) => r,
                Err(strip_err) => {
                    return Err(BlastError::Invalid(format!(
                        "strip_prefix failed for `{}`: {}",
                        path.display(),
                        strip_err
                    )));
                }
            };
            if should_skip(rel) {
                files_skipped += 1;
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            files_checked += 1;
            let dest = destination.join(rel);
            if !dest.is_file() {
                drift.push(rel.to_path_buf());
                continue;
            }
            let src_body = fs::read(path)?;
            let expected = patch_body_if_cargo_toml(rel, src_body);
            let actual = fs::read(&dest)?;
            if expected != actual {
                drift.push(rel.to_path_buf());
            }
        }
    }

    if drift.is_empty() {
        sink.success(format!(
            "canonical snapshot in sync ({} files checked, {} skipped)",
            files_checked, files_skipped
        ));
    } else {
        sink.warn(format!(
            "canonical snapshot has drift: {} file(s) need re-sync",
            drift.len()
        ));
        for p in drift.iter().take(10) {
            sink.warn(format!("  drift: {}", p.display()));
        }
    }

    Ok(Outcome {
        files_copied: 0,
        files_skipped,
        drift_paths: drift,
        destination: destination.to_path_buf(),
    })
}

/// Resolve the default `<blast-root>/../catalyst/` path. Used when the
/// caller does not pass an explicit `--catalyst-path`.
pub fn default_catalyst_path() -> BlastResult<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR may be `<catablast>/blast` or
    // `<catablast>/blast/worktrees/<name>`. Walk up until we find a sibling
    // `catalyst/` checkout.
    let mut probe = manifest_dir.clone();
    loop {
        let candidate = probe.join("catalyst");
        if candidate.is_dir() && candidate.join("Cargo.toml").is_file() {
            return Ok(candidate);
        }
        match probe.parent() {
            Some(p) => probe = p.to_path_buf(),
            None => {
                return Err(BlastError::Invalid(format!(
                    "could not locate catalyst checkout above blast manifest dir {}",
                    manifest_dir.display()
                )));
            }
        }
    }
}

/// Resolve the default destination path inside the blast crate root.
pub fn default_destination() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join("canonical")
}

fn should_skip(rel: &Path) -> bool {
    let s = rel.to_string_lossy();
    let parts: Vec<&str> = s.split('/').collect();

    // Top-level dir excludes
    let top_excluded = match parts.first() {
        Some(p) => matches!(
            *p,
            "target" | "worktrees" | ".git" | "node_modules" | ".claude"
        ),
        None => false, // allow: empty path has no top-level segment to exclude
    };
    if top_excluded {
        return true;
    }

    // frontend/dist anywhere
    if s.contains("/dist/") || s.ends_with("/dist") {
        if parts.iter().any(|p| *p == "frontend") {
            return true;
        }
    }

    // node_modules anywhere
    if parts.iter().any(|p| *p == "node_modules") {
        return true;
    }

    // Specific files
    let basename = match rel.file_name().and_then(|n| n.to_str()) {
        Some(b) => b,
        None => return false,
    };

    matches!(
        basename,
        "Cargo.lock"
            | "CLAUDE.md"
            | "check_errors.sh"
            | ".env"
            | "blast.log"
            | "server.pid"
            | "blast_log.sh"
            | "monitor_log.sh"
    ) || basename.ends_with(".log")
        || basename.ends_with(".tmp")
        || basename.ends_with(".bak")
}

/// If this is the project root Cargo.toml, drop the `[workspace]` stanza
/// (vendored apps aren't workspace anchors) and rename
/// `name = "catalyst"` to `name = "{{project_name}}"`.
fn patch_body_if_cargo_toml(rel: &Path, body: Vec<u8>) -> Vec<u8> {
    if rel != Path::new("Cargo.toml") {
        return body;
    }
    let text = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_utf8_err) => return body, // allow: non-utf8 Cargo.toml is unreachable in catalyst, but if it ever happened we pass through unchanged rather than corrupt bytes
    };
    let mut out = String::with_capacity(text.len());
    let mut skip_workspace_block = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("[workspace]") {
            skip_workspace_block = true;
            continue;
        }
        if skip_workspace_block {
            // Workspace block ends at the first blank line OR next [section].
            if trimmed.is_empty() {
                skip_workspace_block = false;
                continue;
            }
            if trimmed.starts_with('[') {
                skip_workspace_block = false;
                // fall through and emit this line
            } else {
                continue;
            }
        }
        let patched_line = if line.trim_start().starts_with("name = \"catalyst\"")
            || line.trim_start().starts_with("name=\"catalyst\"")
        {
            line.replacen("\"catalyst\"", "\"{{project_name}}\"", 1)
        } else {
            line.to_string()
        };
        out.push_str(&patched_line);
        out.push('\n');
    }
    out.into_bytes()
}

fn plant_gitkeeps(destination: &Path) -> BlastResult<()> {
    for sub in ["storage/blast", "storage/cronjobs", "storage/logs"] {
        let dir = destination.join(sub);
        if !dir.is_dir() {
            continue;
        }
        let mut iter = fs::read_dir(&dir)?;
        let empty = iter.next().is_none();
        if empty {
            let marker = dir.join(".gitkeep");
            fs::write(&marker, "")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::NullSink;
    use tempfile::tempdir;

    fn make_fake_catalyst(root: &Path) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\".\"]\n\n[package]\nname = \"catalyst\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nfoo = \"1\"\n",
        )
        .unwrap();
        fs::write(root.join("src").join("lib.rs"), "// fake catalyst lib\n").unwrap();
        fs::write(root.join("src").join("bootstrap.rs"), "// fake bootstrap\n").unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("target").join("garbage.bin"), b"junk").unwrap();
        fs::write(root.join("Cargo.lock"), "# locked\n").unwrap();
        fs::write(root.join("CLAUDE.md"), "internal\n").unwrap();
        fs::create_dir_all(root.join("storage").join("logs")).unwrap();
        fs::write(
            root.join("storage").join("logs").join("server.log"),
            "noise\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("storage").join("blast")).unwrap();
        fs::write(
            root.join("storage").join("blast").join("server.pid"),
            "1234",
        )
        .unwrap();
    }

    #[test]
    fn sync_canonical_copies_tree() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();
        make_fake_catalyst(tmp_src.path());

        let args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: tmp_dst.path().join("canonical"),
            check: false,
        };
        let mut sink = NullSink;
        let outcome = run(args, &mut sink).expect("sync");
        assert!(outcome.files_copied >= 3, "copied: {}", outcome.files_copied);

        let dest = tmp_dst.path().join("canonical");
        assert!(dest.join("Cargo.toml").is_file());
        assert!(dest.join("src").join("lib.rs").is_file());
        assert!(dest.join("src").join("bootstrap.rs").is_file());

        // Excluded paths should not exist.
        assert!(!dest.join("target").exists(), "target should be excluded");
        assert!(!dest.join("Cargo.lock").exists(), "Cargo.lock excluded");
        assert!(!dest.join("CLAUDE.md").exists(), "CLAUDE.md excluded");
        assert!(
            !dest.join("storage").join("logs").join("server.log").exists(),
            "log files excluded"
        );
        assert!(
            !dest.join("storage").join("blast").join("server.pid").exists(),
            "pid files excluded"
        );
    }

    #[test]
    fn sync_canonical_patches_cargo_toml_name() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();
        make_fake_catalyst(tmp_src.path());

        let args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: tmp_dst.path().join("canonical"),
            check: false,
        };
        let mut sink = NullSink;
        run(args, &mut sink).expect("sync");

        let body = fs::read_to_string(tmp_dst.path().join("canonical").join("Cargo.toml"))
            .expect("read Cargo.toml");
        assert!(
            body.contains(r#"name = "{{project_name}}""#),
            "cargo toml should be templated, got:\n{body}"
        );
        assert!(!body.contains("[workspace]"), "workspace block should be dropped");
    }

    #[test]
    fn sync_canonical_check_mode_detects_drift() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();
        make_fake_catalyst(tmp_src.path());

        // First do a real sync.
        let args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: tmp_dst.path().join("canonical"),
            check: false,
        };
        let mut sink = NullSink;
        run(args, &mut sink).expect("sync");

        // Now mutate the source and run check mode.
        fs::write(
            tmp_src.path().join("src").join("lib.rs"),
            "// MUTATED\n",
        )
        .unwrap();
        let check_args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: tmp_dst.path().join("canonical"),
            check: true,
        };
        let outcome = run(check_args, &mut sink).expect("check");
        assert!(
            !outcome.drift_paths.is_empty(),
            "expected drift, got none"
        );
    }

    #[test]
    fn sync_canonical_check_mode_clean_no_drift() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();
        make_fake_catalyst(tmp_src.path());

        let args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: tmp_dst.path().join("canonical"),
            check: false,
        };
        let mut sink = NullSink;
        run(args, &mut sink).expect("sync");

        let check_args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: tmp_dst.path().join("canonical"),
            check: true,
        };
        let outcome = run(check_args, &mut sink).expect("check");
        assert!(
            outcome.drift_paths.is_empty(),
            "expected no drift after fresh sync, got: {:?}",
            outcome.drift_paths
        );
    }

    #[test]
    fn should_skip_excludes_target_and_lock() {
        assert!(should_skip(Path::new("target/garbage.bin")));
        assert!(should_skip(Path::new("Cargo.lock")));
        assert!(should_skip(Path::new("CLAUDE.md")));
        assert!(should_skip(Path::new(".git/HEAD")));
        assert!(should_skip(Path::new("worktrees/foo/Cargo.toml")));
        assert!(should_skip(Path::new("frontend/node_modules/foo.js")));
        assert!(should_skip(Path::new("storage/logs/server.log")));
        assert!(!should_skip(Path::new("Cargo.toml")));
        assert!(!should_skip(Path::new("src/lib.rs")));
    }

    // --- path-aware sync tests ---

    /// Sync preserves blast-owned paths (e.g. `frontend/`) that are NOT in
    /// CANONICAL_SOURCE_PATHS, even though catalyst has no such directory.
    #[test]
    fn sync_preserves_blast_owned_paths() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();

        // catalyst has src/foo.rs and Cargo.toml only.
        fs::create_dir_all(tmp_src.path().join("src")).unwrap();
        fs::write(tmp_src.path().join("src").join("foo.rs"), "// foo\n").unwrap();
        fs::write(
            tmp_src.path().join("Cargo.toml"),
            "[package]\nname = \"catalyst\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        // templates/canonical/ pre-exists with src/foo.rs AND frontend/index.html.
        let dest = tmp_dst.path().join("canonical");
        fs::create_dir_all(dest.join("src")).unwrap();
        fs::write(dest.join("src").join("foo.rs"), "// foo\n").unwrap();
        fs::create_dir_all(dest.join("frontend")).unwrap();
        fs::write(dest.join("frontend").join("index.html"), "<html/>\n").unwrap();

        let args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: dest.clone(),
            check: false,
        };
        let mut sink = NullSink;
        run(args, &mut sink).expect("sync");

        // src/foo.rs should be present (catalyst owns it).
        assert!(dest.join("src").join("foo.rs").is_file(), "src/foo.rs missing");
        // frontend/index.html must survive — blast-owned, not catalyst-owned.
        assert!(
            dest.join("frontend").join("index.html").is_file(),
            "frontend/index.html was nuked by sync but should be preserved"
        );
    }

    /// Sync removes catalyst-owned files that catalyst deleted (stale files
    /// under catalyst-owned paths are cleaned up).
    #[test]
    fn sync_removes_stale_catalyst_owned_files() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();

        // catalyst has src/foo.rs only — src/bar.rs was deleted.
        fs::create_dir_all(tmp_src.path().join("src")).unwrap();
        fs::write(tmp_src.path().join("src").join("foo.rs"), "// foo\n").unwrap();
        fs::write(
            tmp_src.path().join("Cargo.toml"),
            "[package]\nname = \"catalyst\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        // dest has src/foo.rs AND the stale src/bar.rs.
        let dest = tmp_dst.path().join("canonical");
        fs::create_dir_all(dest.join("src")).unwrap();
        fs::write(dest.join("src").join("foo.rs"), "// foo\n").unwrap();
        fs::write(dest.join("src").join("bar.rs"), "// bar (stale)\n").unwrap();

        let args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: dest.clone(),
            check: false,
        };
        let mut sink = NullSink;
        run(args, &mut sink).expect("sync");

        assert!(dest.join("src").join("foo.rs").is_file(), "foo.rs should remain");
        assert!(
            !dest.join("src").join("bar.rs").exists(),
            "bar.rs should have been removed (catalyst deleted it)"
        );
    }

    /// --check ignores blast-owned paths. Even if templates/canonical/ has
    /// files under frontend/ that catalyst doesn't have, it reports no drift.
    #[test]
    fn check_ignores_blast_owned_paths() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();

        let cargo_content =
            "[package]\nname = \"catalyst\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

        // catalyst has src/foo.rs and Cargo.toml — no frontend/ at all.
        fs::create_dir_all(tmp_src.path().join("src")).unwrap();
        fs::write(tmp_src.path().join("src").join("foo.rs"), "// foo\n").unwrap();
        fs::write(tmp_src.path().join("Cargo.toml"), cargo_content).unwrap();

        // templates/canonical/ has matching src/foo.rs + Cargo.toml (in sync)
        // PLUS blast-owned frontend/page.vue that catalyst will never have.
        let dest = tmp_dst.path().join("canonical");
        fs::create_dir_all(dest.join("src")).unwrap();
        fs::write(dest.join("src").join("foo.rs"), "// foo\n").unwrap();
        // Cargo.toml in dest must match the patched version check would expect.
        // patch_body_if_cargo_toml rewrites name = "catalyst" -> {{project_name}}.
        fs::write(
            dest.join("Cargo.toml"),
            "[package]\nname = \"{{project_name}}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::create_dir_all(dest.join("frontend")).unwrap();
        fs::write(dest.join("frontend").join("page.vue"), "<template/>\n").unwrap();

        let check_args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: dest.clone(),
            check: true,
        };
        let mut sink = NullSink;
        let outcome = run(check_args, &mut sink).expect("check");
        assert!(
            outcome.drift_paths.is_empty(),
            "frontend/ is blast-owned, must not count as drift; got: {:?}",
            outcome.drift_paths
        );
    }

    /// --check still detects drift in catalyst-owned paths (content mismatch).
    #[test]
    fn check_detects_drift_in_catalyst_owned_paths() {
        let tmp_src = tempdir().unwrap();
        let tmp_dst = tempdir().unwrap();

        // catalyst has src/foo.rs with content "A".
        fs::create_dir_all(tmp_src.path().join("src")).unwrap();
        fs::write(tmp_src.path().join("src").join("foo.rs"), "// A\n").unwrap();
        fs::write(
            tmp_src.path().join("Cargo.toml"),
            "[package]\nname = \"catalyst\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        // templates/canonical/ has src/foo.rs with content "B" (stale).
        let dest = tmp_dst.path().join("canonical");
        fs::create_dir_all(dest.join("src")).unwrap();
        fs::write(dest.join("src").join("foo.rs"), "// B\n").unwrap();

        let check_args = Args {
            catalyst_path: Some(tmp_src.path().to_path_buf()),
            destination: dest.clone(),
            check: true,
        };
        let mut sink = NullSink;
        let outcome = run(check_args, &mut sink).expect("check");
        assert!(
            !outcome.drift_paths.is_empty(),
            "expected drift on src/foo.rs content mismatch"
        );
        let drift_strs: Vec<String> =
            outcome.drift_paths.iter().map(|p| p.display().to_string()).collect();
        assert!(
            drift_strs.iter().any(|s| s.contains("foo.rs")),
            "drift should include foo.rs, got: {:?}",
            drift_strs
        );
    }
}
