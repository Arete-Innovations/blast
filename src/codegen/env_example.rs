//! Emits `.env.example` at the project root from the `EnvSpec` section of
//! `storage/blast/state/app.ron`.
//!
//! Output format:
//! ```text
//! # AUTO-GENERATED from storage/blast/state/app.ron @ <hash>
//! #
//! # Do not edit by hand. Run `blast gen all` after mutating state.
//!
//! # <comment>
//! NAME=default_or_changeme
//! ```

use crate::codegen::header;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, Sink, SinkExt};
use crate::state::{AppPolicySection, EnvSpecState, EnvVarSpec};
use std::fs;
use std::path::{Path, PathBuf};

const ENV_EXAMPLE_RELATIVE: &str = ".env.example";
const SENSITIVE_PLACEHOLDER: &str = "<changeme>";

pub struct EmitReport {
    pub written: Option<PathBuf>,
}

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    _progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    let state_dir = project_root.join("storage/blast/state");
    let env_spec = load_env_spec(&state_dir)?;

    match env_spec {
        Some(spec) if !spec.vars.is_empty() => {
            let path = emit(project_root, &spec)?;
            sink.info(format!("emitted {}", path.display()));
            Ok(EmitReport { written: Some(path) })
        }
        Some(_spec_empty) => {
            sink.warn(
                "no env spec declared in app.ron — skipping .env.example".to_string(),
            );
            Ok(EmitReport { written: None })
        }
        None => {
            sink.warn(
                "no env spec declared in app.ron — skipping .env.example".to_string(),
            );
            Ok(EmitReport { written: None })
        }
    }
}

fn load_env_spec(state_dir: &Path) -> BlastResult<Option<EnvSpecState>> {
    let app_state = match crate::state::load_app(state_dir) {
        Ok(s) => s,
        Err(BlastError::Io(io_err)) if io_err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    extract_env_spec(app_state.sections.get("env_spec"))
}

fn extract_env_spec(
    section: Option<&AppPolicySection>,
) -> BlastResult<Option<EnvSpecState>> {
    let section = match section {
        Some(s) => s,
        None => return Ok(None),
    };
    match section {
        AppPolicySection::EnvSpec(state) => Ok(Some(state.clone())),
        AppPolicySection::FeLint(_) => Ok(None),
        AppPolicySection::Admin(_) => Ok(None),
        AppPolicySection::Fuses(_) => Ok(None),
        AppPolicySection::Services(_) => Ok(None),
        AppPolicySection::Defaults(_) => Ok(None),
    }
}

fn emit(project_root: &Path, spec: &EnvSpecState) -> BlastResult<PathBuf> {
    let marker = marker_for_env(project_root)?;
    let body = render(spec);
    let target = project_root.join(ENV_EXAMPLE_RELATIVE);
    fs::write(&target, format!("{}{}", marker, body))?;
    Ok(target)
}

/// Build a `#`-comment header from the standard app-state marker.
///
/// `header::marker_for_app` returns `// AUTO-GENERATED ...` lines.
/// `.env.example` uses shell-comment syntax, so we replace `// ` with `# `
/// and standalone `//` with `#`.
fn marker_for_env(project_root: &Path) -> BlastResult<String> {
    let rs_marker = header::marker_for_app(project_root)?;
    let converted = rs_marker
        .lines()
        .map(convert_line)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("{}\n", converted))
}

fn convert_line(line: &str) -> String {
    match line.strip_prefix("// ") {
        Some(rest) => format!("# {}", rest),
        None => match line {
            "//" => "#".to_string(),
            other => other.to_string(),
        },
    }
}

fn render(spec: &EnvSpecState) -> String {
    let mut out = String::new();
    for (name, var) in &spec.vars {
        render_var(&mut out, name, var);
    }
    out
}

fn render_var(out: &mut String, name: &str, var: &EnvVarSpec) {
    match &var.comment {
        Some(comment) => out.push_str(&format!("# {}\n", comment)),
        None => {}
    }
    let value = if var.sensitive {
        SENSITIVE_PLACEHOLDER.to_string()
    } else {
        var.default.clone()
    };
    out.push_str(&format!("{}={}\n", name, value));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::NullProgress;
    use crate::io::recorder::RecorderSink;
    use crate::state::{AppPolicySection, AppState, EnvSpecState, EnvVarSpec};
    use indexmap::IndexMap;
    use std::fs;
    use tempfile::TempDir;

    fn make_state_with_env_spec() -> AppState {
        let mut vars: IndexMap<String, EnvVarSpec> = IndexMap::new();
        vars.insert(
            "DATABASE_URL".to_string(),
            EnvVarSpec {
                default: "postgres://localhost/myapp".to_string(),
                comment: Some("Postgres connection string".to_string()),
                sensitive: false,
            },
        );
        vars.insert(
            "SESSION_SIGNING_KEY".to_string(),
            EnvVarSpec {
                default: "".to_string(),
                comment: Some("32-byte hex secret for session tokens".to_string()),
                sensitive: true,
            },
        );
        vars.insert(
            "APP_PORT".to_string(),
            EnvVarSpec {
                default: "8080".to_string(),
                comment: None,
                sensitive: false,
            },
        );
        let spec = EnvSpecState { vars };
        let mut state = AppState::new();
        state
            .sections
            .insert("env_spec".to_string(), AppPolicySection::EnvSpec(spec));
        state
    }

    fn write_app_ron(dir: &TempDir, state: &AppState) {
        let state_dir = dir.path().join("storage/blast/state");
        fs::create_dir_all(&state_dir).unwrap();
        let ron = ron::ser::to_string_pretty(
            state,
            ron::ser::PrettyConfig::new().struct_names(true),
        )
        .unwrap();
        fs::write(state_dir.join("app.ron"), ron).unwrap();
    }

    #[test]
    fn emits_env_example_with_three_vars() {
        let dir = TempDir::new().unwrap();
        let state = make_state_with_env_spec();
        write_app_ron(&dir, &state);

        let mut sink = RecorderSink::new();
        let mut progress = NullProgress;
        let report = run(dir.path(), &mut sink, &mut progress).unwrap();

        let written_path = report.written.unwrap();
        assert_eq!(written_path, dir.path().join(".env.example"));

        let content = fs::read_to_string(&written_path).unwrap();

        assert!(content.contains("# AUTO-GENERATED from"));
        assert!(content.contains("storage/blast/state/app.ron"));
        let first_line = content.lines().next().expect("file has at least one line");
        assert!(
            first_line.starts_with('#'),
            "header first line must start with #, got: {first_line}"
        );

        assert!(content.contains("# Postgres connection string\nDATABASE_URL=postgres://localhost/myapp"));
        assert!(content.contains("# 32-byte hex secret for session tokens\nSESSION_SIGNING_KEY=<changeme>"));
        assert!(content.contains("APP_PORT=8080"));
    }

    #[test]
    fn sensitive_var_shows_changeme_not_default() {
        let mut vars: IndexMap<String, EnvVarSpec> = IndexMap::new();
        vars.insert(
            "SECRET".to_string(),
            EnvVarSpec {
                default: "actual-secret-value".to_string(),
                comment: None,
                sensitive: true,
            },
        );
        let spec = EnvSpecState { vars };
        let rendered = render(&spec);
        assert!(rendered.contains("SECRET=<changeme>"));
        assert!(!rendered.contains("actual-secret-value"));
    }

    #[test]
    fn skips_when_no_env_spec_section() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new();
        write_app_ron(&dir, &state);

        let mut sink = RecorderSink::new();
        let mut progress = NullProgress;
        let report = run(dir.path(), &mut sink, &mut progress).unwrap();

        assert!(report.written.is_none());
        let env_example = dir.path().join(".env.example");
        assert!(!env_example.exists());
    }
}
