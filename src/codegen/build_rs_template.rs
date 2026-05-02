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
        assert!(src.contains("src/transport/leptos/components/generated"));
        assert!(src.contains("src/transport/leptos/data/generated"));
        assert!(src.contains("src/transport/leptos/nav/generated"));
        assert!(src.contains("src/transport/leptos/pages/generated"));
        assert!(src.contains("src/transport/leptos/routes/generated"));
        assert!(src.contains("src/transport/leptos/validators/generated"));
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

    fn simulate_check(root: &std::path::Path) -> Result<(), String> {
        let watched_dirs: &[&str] = &[
            "src/structs/generated",
            "src/models/generated",
            "src/routines/generated",
            "src/flows/generated",
            "src/transport/http/generated",
            "src/transport/ws/generated",
            "src/transport/leptos/components/generated",
            "src/transport/leptos/data/generated",
            "src/transport/leptos/nav/generated",
            "src/transport/leptos/pages/generated",
            "src/transport/leptos/routes/generated",
            "src/transport/leptos/validators/generated",
        ];

        for rel_dir in watched_dirs {
            let dir_path = root.join(rel_dir);
            if !dir_path.exists() {
                continue;
            }
            let entries = match fs::read_dir(&dir_path) {
                Ok(e) => e,
                Err(err) => return Err(format!("read_dir {}: {}", dir_path.display(), err)),
            };
            for entry in entries.flatten() {
                let path = entry.path();
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
            if hash_part.chars().all(|c| c.is_ascii_hexdigit()) && !hash_part.is_empty() {
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
