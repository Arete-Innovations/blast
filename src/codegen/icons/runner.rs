//! Pipeline driver for the icons codegen pass.
//!
//! Reads the `Icons` section from `app.ron` and emits
//! `frontend/src/generated/icons.ts` with the standard hash-marker
//! header. Falls back to a default registry when the section is absent.

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::header;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state;
use crate::state::{AppPolicySection, AppState, IconConfig};

use super::emit::emit_icons_ts;

/// Report of the file written during an icons codegen run.
#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Option<PathBuf>,
}

const STEP_LABEL: &str = "icons codegen";
const ICONS_RELATIVE: &str = "frontend/src/generated/icons.ts";

/// Top-level entry point. Reads `app.ron`, extracts the `Icons` section,
/// and writes the registry file with the marker header. Falls back to a
/// default registry when no `Icons` section is present.
pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);
    let result = run_inner(project_root, sink);
    match &result {
        Ok(_report) => progress.step_done(STEP_LABEL),
        Err(err) => progress.step_fail(STEP_LABEL, err.to_string()),
    }
    result
}

fn run_inner(project_root: &Path, sink: &mut dyn Sink) -> BlastResult<EmitReport> {
    let state_dir = project_root.join("storage").join("blast").join("state");
    let app_state = load_app_or_default(&state_dir)?;
    let icons = extract_icons(&app_state);

    let app_marker = header::marker_for_app(project_root)?;
    let body = format!("{}{}", app_marker, emit_icons_ts(&icons));

    let target = project_root.join(ICONS_RELATIVE);
    let parent = match target.parent() {
        Some(p) => p,
        None => {
            return Err(BlastError::Invalid(format!(
                "icons codegen target has no parent: {}",
                target.display()
            )))
        }
    };
    fs::create_dir_all(parent)?;
    fs::write(&target, body)?;
    sink.info(format!("emitted {}", target.display()));
    Ok(EmitReport { written: Some(target) })
}

fn load_app_or_default(state_dir: &Path) -> BlastResult<AppState> {
    match state::load_app(state_dir) {
        Ok(s) => Ok(s),
        Err(BlastError::Io(io_err)) => match io_err.kind() {
            std::io::ErrorKind::NotFound => Ok(AppState::default()),
            other_kind => Err(BlastError::Io(std::io::Error::new(other_kind, io_err))),
        },
        Err(other) => Err(other),
    }
}

fn extract_icons(app: &AppState) -> IconConfig {
    for (_k, section) in &app.sections {
        match section {
            AppPolicySection::Icons(cfg) => return cfg.clone(),
            AppPolicySection::FeLint(_)
            | AppPolicySection::Admin(_)
            | AppPolicySection::Fuses(_)
            | AppPolicySection::Services(_)
            | AppPolicySection::EnvSpec(_)
            | AppPolicySection::Defaults(_)
            | AppPolicySection::Nav(_)
            | AppPolicySection::Pages(_)
            | AppPolicySection::Theme(_) => continue,
        }
    }
    IconConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::NullProgress;
    use crate::io::recorder::RecorderSink;
    use crate::state::{AppPolicySection, AppState};
    use std::fs;
    use tempfile::TempDir;

    fn write_app_ron_with_default_icons(dir: &TempDir) {
        let mut state = AppState::new();
        state.sections.insert(
            crate::state::app::ICONS_SECTION_KEY.to_string(),
            AppPolicySection::Icons(IconConfig::default()),
        );
        let state_dir = dir.path().join("storage/blast/state");
        fs::create_dir_all(&state_dir).unwrap();
        let ron = ron::ser::to_string_pretty(
            &state,
            ron::ser::PrettyConfig::new().struct_names(true),
        )
        .unwrap();
        fs::write(state_dir.join("app.ron"), ron).unwrap();
    }

    #[test]
    fn run_writes_file_with_marker_header() {
        let dir = TempDir::new().unwrap();
        write_app_ron_with_default_icons(&dir);

        let mut sink = RecorderSink::new();
        let mut progress = NullProgress;
        let report = run(dir.path(), &mut sink, &mut progress).unwrap();

        let path = report.written.expect("file should have been written");
        assert_eq!(path, dir.path().join(ICONS_RELATIVE));
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("// AUTO-GENERATED from "));
        assert!(content.contains("storage/blast/state/app.ron"));
        assert!(content.contains("export const IC = {"));
        assert!(content.contains("'pi pi-home'"));
        assert!(content.contains("export type IconName = keyof typeof IC"));
    }

    #[test]
    fn run_falls_back_to_default_when_section_missing() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new();
        let state_dir = dir.path().join("storage/blast/state");
        fs::create_dir_all(&state_dir).unwrap();
        let ron = ron::ser::to_string_pretty(
            &state,
            ron::ser::PrettyConfig::new().struct_names(true),
        )
        .unwrap();
        fs::write(state_dir.join("app.ron"), ron).unwrap();

        let mut sink = RecorderSink::new();
        let mut progress = NullProgress;
        let report = run(dir.path(), &mut sink, &mut progress).unwrap();

        let path = report.written.unwrap();
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("'pi pi-home'"));
        assert!(content.contains("'pi pi-refresh'"));
    }
}
