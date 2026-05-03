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
        assert!(src.contains("check_transport_handler_ctx"));
    }

    #[test]
    fn render_template_has_no_stale_detection() {
        let src = render_template();
        assert!(!src.contains("AUTO-GENERATED"), "stale-detection killed: see SPEC_CODEGEN");
        assert!(!src.contains("blake3"), "no hash-checking remains");
        assert!(!src.contains("WATCHED_DIRS"), "no walk-and-check anymore");
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
}
