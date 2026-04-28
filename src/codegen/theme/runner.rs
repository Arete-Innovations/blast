//! Pipeline driver for the theme codegen pass.
//!
//! Reads the `Theme` section from `app.ron` (falling back to a default
//! theme if absent), then emits two files: `tokens.css` for the
//! design-token catalog and `primevue.ts` for the PrimeVue Aura preset
//! overlay. Both carry a hash-marker header keyed off `app.ron`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{primevue::emit_primevue_ts, tokens::emit_tokens_css};
use crate::{
    codegen::header,
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state,
    state::{AppPolicySection, AppState, ThemeConfig},
};

/// Report of files written during a theme codegen run.
#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
}

const STEP_LABEL: &str = "theme codegen";
const TOKENS_RELATIVE: &str = "frontend/src/generated/styles/tokens.css";
const PRIMEVUE_RELATIVE: &str = "frontend/src/generated/plugins/primevue.ts";

/// Top-level entry point. Reads `app.ron`, extracts the `Theme` section,
/// and writes both files with the hash-marker header. Falls back to a
/// default theme when no `Theme` section is present, mirroring how other
/// codegen passes treat optional state sections.
pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
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
    let theme = extract_theme(&app_state);

    let app_marker = header::marker_for_app(project_root)?;
    let css_marker = ts_marker_to_css(&app_marker);

    let mut report = EmitReport::default();
    let css_body = format!("{}{}", css_marker, emit_tokens_css(&theme));
    write_emitted(project_root, TOKENS_RELATIVE, &css_body, &mut report, sink)?;

    let ts_body = format!("{}{}", app_marker, emit_primevue_ts(&theme));
    write_emitted(project_root, PRIMEVUE_RELATIVE, &ts_body, &mut report, sink)?;

    Ok(report)
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

fn extract_theme(app: &AppState) -> ThemeConfig {
    for (_k, section) in &app.sections {
        match section {
            AppPolicySection::Theme(theme) => return theme.clone(),
            AppPolicySection::FeLint(_)
            | AppPolicySection::Admin(_)
            | AppPolicySection::Fuses(_)
            | AppPolicySection::Services(_)
            | AppPolicySection::EnvSpec(_)
            | AppPolicySection::Defaults(_)
            | AppPolicySection::Nav(_)
            | AppPolicySection::Pages(_)
            | AppPolicySection::Icons(_) => continue,
        }
    }
    ThemeConfig::default()
}

/// Convert the canonical TS-shaped marker into a CSS-shaped block
/// comment. The TS marker is a sequence of `// ...` lines plus a blank
/// line; we wrap them inside `/*` ... ` */` so the CSS body stays
/// parseable while still embedding the path + hash on the first line.
pub(crate) fn ts_marker_to_css(ts_marker: &str) -> String {
    let trimmed = ts_marker.trim_end_matches('\n');
    let mut out = String::new();
    out.push_str("/*\n");
    for line in trimmed.lines() {
        match line.strip_prefix("// ") {
            Some(rest) => out.push_str(&format!(" * {rest}\n")),
            None => {
                if line == "//" {
                    out.push_str(" *\n");
                } else {
                    out.push_str(&format!(" * {line}\n"));
                }
            }
        }
    }
    out.push_str(" */\n\n");
    out
}

fn write_emitted(project_root: &Path, relative: &str, body: &str, report: &mut EmitReport, sink: &mut dyn Sink) -> BlastResult<()> {
    let target = project_root.join(relative);
    let parent = match target.parent() {
        Some(p) => p,
        None => return Err(BlastError::Invalid(format!("theme codegen target has no parent: {}", target.display()))),
    };
    fs::create_dir_all(parent)?;
    fs::write(&target, body)?;
    report.written.push(target.clone());
    sink.info(format!("emitted {}", target.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::{
        io::{null::NullProgress, recorder::RecorderSink},
        state::{AppPolicySection, AppState},
    };

    fn write_app_ron_with_default_theme(dir: &TempDir) {
        let mut state = AppState::new();
        state.sections.insert(crate::state::app::THEME_SECTION_KEY.to_string(), AppPolicySection::Theme(ThemeConfig::default()));
        let state_dir = dir.path().join("storage/blast/state");
        fs::create_dir_all(&state_dir).unwrap();
        let ron = ron::ser::to_string_pretty(&state, ron::ser::PrettyConfig::new().struct_names(true)).unwrap();
        fs::write(state_dir.join("app.ron"), ron).unwrap();
    }

    #[test]
    fn run_emits_both_files_with_headers() {
        let dir = TempDir::new().unwrap();
        write_app_ron_with_default_theme(&dir);

        let mut sink = RecorderSink::new();
        let mut progress = NullProgress;
        let report = run(dir.path(), &mut sink, &mut progress).unwrap();

        assert_eq!(report.written.len(), 2);
        let tokens = dir.path().join(TOKENS_RELATIVE);
        let primevue = dir.path().join(PRIMEVUE_RELATIVE);
        assert!(tokens.exists());
        assert!(primevue.exists());

        let css = fs::read_to_string(&tokens).unwrap();
        assert!(css.starts_with("/*\n"));
        assert!(css.contains("AUTO-GENERATED from"));
        assert!(css.contains("storage/blast/state/app.ron"));
        assert!(css.contains("@layer app {"));
        assert!(css.contains("--app-fs-md: 1rem;"));

        let ts = fs::read_to_string(&primevue).unwrap();
        assert!(ts.starts_with("// AUTO-GENERATED from "));
        assert!(ts.contains("definePreset(Aura,"));
        assert!(ts.contains("'#ffffff'"));
        assert!(ts.contains("'#0a0a0a'"));
    }

    #[test]
    fn run_falls_back_to_default_theme_when_section_missing() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new();
        let state_dir = dir.path().join("storage/blast/state");
        fs::create_dir_all(&state_dir).unwrap();
        let ron = ron::ser::to_string_pretty(&state, ron::ser::PrettyConfig::new().struct_names(true)).unwrap();
        fs::write(state_dir.join("app.ron"), ron).unwrap();

        let mut sink = RecorderSink::new();
        let mut progress = NullProgress;
        run(dir.path(), &mut sink, &mut progress).unwrap();

        let css = fs::read_to_string(dir.path().join(TOKENS_RELATIVE)).unwrap();
        assert!(css.contains("--app-fs-md: 1rem;"));
    }

    #[test]
    fn ts_marker_to_css_wraps_in_block_comment() {
        let ts_marker = "// AUTO-GENERATED from app.ron @ deadbeef\n//\n// Do not edit.\n\n";
        let css = ts_marker_to_css(ts_marker);
        assert!(css.starts_with("/*\n"));
        assert!(css.contains(" * AUTO-GENERATED from app.ron @ deadbeef\n"));
        assert!(css.contains(" *\n"));
        assert!(css.contains(" * Do not edit.\n"));
        assert!(css.contains(" */\n\n"));
    }
}
