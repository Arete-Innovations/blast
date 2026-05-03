use std::{fs, path::PathBuf};

use crate::error::BlastResult;

pub struct Args {
    pub project_root: PathBuf,
}

pub struct Outcome {
    pub written: PathBuf,
    pub action: WriteAction,
}

pub enum WriteAction {
    Created,
    Overwritten,
}

pub fn run(args: Args) -> BlastResult<Outcome> {
    let dest = args.project_root.join("build.rs");
    let action = if dest.exists() { WriteAction::Overwritten } else { WriteAction::Created };
    fs::write(&dest, render_template())?;
    Ok(Outcome { written: dest, action })
}

pub fn render_template() -> &'static str {
    include_str!("build_rs_template_src.rs.tmpl")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn render_template_is_nonempty() {
        let src = render_template();
        assert!(!src.is_empty());
        assert!(src.contains("fn main()"));
        assert!(src.contains("check_state_hashes"));
    }

    #[test]
    fn run_creates_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let args = Args { project_root: dir.path().to_path_buf() };
        let outcome = run(args).expect("run");
        assert!(outcome.written.exists());
        let written = fs::read_to_string(&outcome.written).expect("read");
        assert!(written.contains("fn main()"));
        assert!(matches!(outcome.action, WriteAction::Created));
    }

    #[test]
    fn run_overwrites_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("build.rs");
        fs::write(&dest, "// old").expect("write seed");
        let args = Args { project_root: dir.path().to_path_buf() };
        let outcome = run(args).expect("run");
        let written = fs::read_to_string(&outcome.written).expect("read");
        assert!(written.contains("fn main()"));
        assert!(matches!(outcome.action, WriteAction::Overwritten));
    }

    #[test]
    fn emitted_build_rs_contains_watched_dirs() {
        let src = render_template();
        assert!(src.contains("src/structs/generated"));
        assert!(src.contains("src/models/generated"));
        assert!(src.contains("src/routines/generated"));
        assert!(src.contains("src/flows/generated"));
        assert!(src.contains("src/transport/http/generated"));
        assert!(src.contains("src/transport/ws/generated"));
        assert!(src.contains("src/views/components/generated"));
        assert!(src.contains("src/transport/leptos/data/generated"));
        assert!(src.contains("src/transport/leptos/pages/generated"));
        assert!(src.contains("src/transport/leptos/routes/generated"));
        assert!(src.contains("\"tests\""), "tests dir watched for tests/route_alignment_generated.rs marker");
        assert!(src.contains("storage/blast/state/"));
    }

    #[test]
    fn emitted_build_rs_references_blake3() {
        let src = render_template();
        assert!(src.contains("blake3"));
    }

    #[test]
    fn round_trip_pass() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state").join("resources");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("users.ron");
        fs::write(&state_file, b"ResourceState(schema_version: 1)").expect("write state");

        let mut hasher = blake3::Hasher::new();
        let bytes = fs::read(&state_file).expect("read state");
        hasher.update(&bytes);
        let hash = hasher.finalize().to_hex().to_string();

        let gen_dir = dir.path().join("src").join("structs").join("generated");
        fs::create_dir_all(&gen_dir).expect("create gen dir");
        let gen_file = gen_dir.join("users.rs");
        let mut f = fs::File::create(&gen_file).expect("create gen file");
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ {}", hash).expect("write header");
        writeln!(f, "// re-run: blast gen all").expect("write line");
        writeln!(f, "pub struct User;").expect("write struct");

        let result = simulate_check(dir.path());
        assert!(result.is_ok(), "expected pass: {:?}", result.err());
    }

    #[test]
    fn round_trip_fail_on_changed_state() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state").join("resources");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("users.ron");
        fs::write(&state_file, b"ResourceState(schema_version: 1)").expect("write state");

        let stale_hash = "deadbeef00000000000000000000000000000000000000000000000000000000";

        let gen_dir = dir.path().join("src").join("structs").join("generated");
        fs::create_dir_all(&gen_dir).expect("create gen dir");
        let gen_file = gen_dir.join("users.rs");
        let mut f = fs::File::create(&gen_file).expect("create gen file");
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ {}", stale_hash).expect("write header");
        writeln!(f, "pub struct User;").expect("write struct");

        let result = simulate_check(dir.path());
        assert!(result.is_err(), "expected fail on stale hash");
    }

    #[test]
    fn round_trip_fail_on_stale_routines_layer() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state").join("resources");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("users.ron");
        fs::write(&state_file, b"ResourceState(schema_version: 1)").expect("write state");

        let stale_hash = "deadbeef00000000000000000000000000000000000000000000000000000000";

        let gen_dir = dir.path().join("src").join("routines").join("generated").join("users");
        fs::create_dir_all(&gen_dir).expect("create routines gen dir");
        let gen_file = gen_dir.join("create.rs");
        let mut f = fs::File::create(&gen_file).expect("create routines gen file");
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ {}", stale_hash).expect("write header");
        writeln!(f, "pub async fn run() {{}}").expect("write fn");

        let result = simulate_check(dir.path());
        assert!(result.is_err(), "FIX-031: routines/generated must be in WATCHED_DIRS — stale hash must trip the check");
    }

    #[test]
    fn round_trip_fail_on_stale_leptos_pages_layer() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state").join("resources");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("users.ron");
        fs::write(&state_file, b"ResourceState(schema_version: 1)").expect("write state");

        let stale_hash = "deadbeef00000000000000000000000000000000000000000000000000000000";

        let gen_dir = dir.path().join("src").join("transport").join("leptos").join("pages").join("generated");
        fs::create_dir_all(&gen_dir).expect("create leptos pages gen dir");
        let gen_file = gen_dir.join("users.rs");
        let mut f = fs::File::create(&gen_file).expect("create leptos pages gen file");
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ {}", stale_hash).expect("write header");
        writeln!(f, "pub fn UsersPage() {{}}").expect("write fn");

        let result = simulate_check(dir.path());
        assert!(result.is_err(), "FIX-031: leptos/pages/generated must be in WATCHED_DIRS — stale hash must trip the check");
    }

    #[test]
    fn round_trip_fail_on_stale_tests_route_alignment() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("app.ron");
        fs::write(&state_file, b"AppState(schema_version: 1)").expect("write state");

        let stale_hash = "deadbeef00000000000000000000000000000000000000000000000000000000";

        let tests_dir = dir.path().join("tests");
        fs::create_dir_all(&tests_dir).expect("create tests dir");
        let gen_file = tests_dir.join("route_alignment_generated.rs");
        let mut f = fs::File::create(&gen_file).expect("create alignment file");
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/app.ron @ {}", stale_hash).expect("write header");
        writeln!(f, "#[test] fn dummy() {{}}").expect("write dummy");

        let result = simulate_check(dir.path());
        assert!(result.is_err(), "FIX-032: tests/ must be in WATCHED_DIRS so route_alignment_generated.rs hash is checked");
    }

    #[test]
    fn tests_dir_skips_user_authored_files_without_marker() {
        let dir = tempfile::tempdir().expect("tempdir");

        let tests_dir = dir.path().join("tests");
        fs::create_dir_all(&tests_dir).expect("create tests dir");
        let user_file = tests_dir.join("auth_email_normalize.rs");
        fs::write(&user_file, b"#[test]\nfn placeholder() {}\n").expect("write user test");

        let result = simulate_check(dir.path());
        assert!(result.is_ok(), "user-authored tests/ files without AUTO-GENERATED marker must be skipped");
    }

    #[test]
    fn truncated_marker_hash_treated_as_no_marker_not_stale() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state").join("resources");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("users.ron");
        fs::write(&state_file, b"ResourceState(schema_version: 1)").expect("write state");

        let gen_dir = dir.path().join("src").join("structs").join("generated");
        fs::create_dir_all(&gen_dir).expect("create gen dir");
        let gen_file = gen_dir.join("users.rs");
        let mut f = fs::File::create(&gen_file).expect("create gen file");
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ deadbeef").expect("write header");
        writeln!(f, "pub struct User;").expect("write struct");

        let result = simulate_check(dir.path());
        assert!(result.is_ok(), "FIX-038: 8-char hash must NOT parse as a valid marker → file skipped, no false stale-hash panic");
    }

    #[test]
    fn non_hex_marker_hash_treated_as_no_marker() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state").join("resources");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("users.ron");
        fs::write(&state_file, b"ResourceState(schema_version: 1)").expect("write state");

        let gen_dir = dir.path().join("src").join("structs").join("generated");
        fs::create_dir_all(&gen_dir).expect("create gen dir");
        let gen_file = gen_dir.join("users.rs");
        let mut f = fs::File::create(&gen_file).expect("create gen file");
        let bad = "g".repeat(64);
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ {bad}").expect("write header");
        writeln!(f, "pub struct User;").expect("write struct");

        let result = simulate_check(dir.path());
        assert!(result.is_ok(), "FIX-038: 64 'g' chars (non-hex) must NOT parse as a valid marker");
    }

    #[test]
    fn round_trip_fail_on_stale_leptos_components_nested_subdir() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");

        let state_dir = dir.path().join("storage").join("blast").join("state").join("resources");
        fs::create_dir_all(&state_dir).expect("create state dir");
        let state_file = state_dir.join("users.ron");
        fs::write(&state_file, b"ResourceState(schema_version: 1)").expect("write state");

        let stale_hash = "deadbeef00000000000000000000000000000000000000000000000000000000";

        let gen_dir = dir.path().join("src").join("views").join("components").join("generated").join("forms");
        fs::create_dir_all(&gen_dir).expect("create nested forms gen dir");
        let gen_file = gen_dir.join("users_form.rs");
        let mut f = fs::File::create(&gen_file).expect("create nested gen file");
        writeln!(f, "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ {}", stale_hash).expect("write header");
        writeln!(f, "pub fn UsersForm() {{}}").expect("write fn");

        let result = simulate_check(dir.path());
        assert!(result.is_err(), "FIX-031: recursive walk must descend into components/generated/forms/ subdir");
    }

    fn simulate_check(root: &std::path::Path) -> Result<(), String> {
        let watched_dirs: &[&str] = &[
            "src/structs/generated",
            "src/models/generated",
            "src/routines/generated",
            "src/flows/generated",
            "src/transport/http/generated",
            "src/transport/ws/generated",
            "src/views/components/generated",
            "src/transport/leptos/data/generated",
            "src/transport/leptos/pages/generated",
            "src/transport/leptos/routes/generated",
            "tests",
        ];

        for rel_dir in watched_dirs {
            let dir_path = root.join(rel_dir);
            if !dir_path.exists() {
                continue;
            }
            walk_and_check(root, &dir_path)?;
        }
        Ok(())
    }

    fn walk_and_check(root: &std::path::Path, dir: &std::path::Path) -> Result<(), String> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(err) => return Err(format!("read_dir {}: {}", dir.display(), err)),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_and_check(root, &path)?;
                continue;
            }
            let ext = path.extension().map(|e| e.to_string_lossy().to_string());
            if ext.as_deref() != Some("rs") {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(err) => return Err(format!("read {}: {}", path.display(), err)),
            };
            let (state_path_str, marker_hash) = match parse_marker(&content) {
                Some(pair) => pair,
                None => continue,
            };
            let state_file = root.join(&state_path_str);
            if !state_file.exists() {
                return Err(format!("state file '{}' missing; was it deleted? regen with 'blast gen all'", state_path_str));
            }
            let actual_hash = compute_hash(&state_file).map_err(|err| format!("hash {}: {}", state_file.display(), err))?;
            if actual_hash != marker_hash {
                return Err(format!(
                    "state file '{}' changed since last regen — run 'blast gen all'\n  expected hash: {}\n  actual hash:   {}",
                    state_path_str, marker_hash, actual_hash
                ));
            }
        }
        Ok(())
    }

    fn parse_marker(content: &str) -> Option<(String, String)> {
        for line in content.lines().take(10) {
            let trimmed = line.trim();
            if !trimmed.starts_with("// AUTO-GENERATED from ") {
                continue;
            }
            let rest = &trimmed["// AUTO-GENERATED from ".len()..];
            let at_pos = rest.rfind(" @ ")?;
            let path_part = rest[..at_pos].trim().to_string();
            let hash_part = rest[at_pos + 3..].trim().to_string();
            if hash_part.len() == 64 && hash_part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some((path_part, hash_part));
            }
        }
        None
    }

    fn compute_hash(path: &std::path::Path) -> Result<String, std::io::Error> {
        use std::io::Read;
        let mut file = fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; 65536];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize().to_hex().to_string())
    }
}
