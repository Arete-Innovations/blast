use crate::database::migration_wizard::spec::{MigrationSpec, Outcome};
use crate::database::migration_wizard::sql;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, Sink};
use crate::io::{ProgressExt, SinkExt};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn run(
    spec: MigrationSpec,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<Outcome> {
    progress.step_start("creating migration files");

    let migration_name = spec.migration_name();
    let (up_sql, down_sql) = render_sql(&spec);

    let (up_path, down_path) = match invoke_diesel_generate(&migration_name) {
        Ok(paths) => paths,
        Err(err) => {
            progress.step_fail("creating migration files", err.to_string());
            return Err(err);
        }
    };

    if let Err(err) = fs::write(&up_path, &up_sql) {
        progress.step_fail("creating migration files", err.to_string());
        return Err(err.into());
    }

    if let Err(err) = fs::write(&down_path, &down_sql) {
        progress.step_fail("creating migration files", err.to_string());
        return Err(err.into());
    }

    sink.success(format!(
        "migration '{}' written:\n  - {}\n  - {}",
        migration_name,
        up_path.display(),
        down_path.display()
    ));
    progress.step_done("creating migration files");

    Ok(Outcome {
        migration_name,
        up_path,
        down_path,
    })
}

fn render_sql(spec: &MigrationSpec) -> (String, String) {
    match spec {
        MigrationSpec::Custom(c) => (c.up_sql.clone(), c.down_sql.clone()),
        MigrationSpec::NewTable(n) => (sql::render_new_table_up(n), sql::render_new_table_down(n)),
        MigrationSpec::AlterTable(a) => (
            sql::render_alter_table_up(a),
            sql::render_alter_table_down(a),
        ),
    }
}

fn invoke_diesel_generate(migration_name: &str) -> BlastResult<(PathBuf, PathBuf)> {
    let output = Command::new("diesel")
        .args(["migration", "generate", migration_name])
        .output()
        .map_err(|e| BlastError::Subprocess {
            cmd: "diesel migration generate".to_string(),
            detail: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(BlastError::Subprocess {
            cmd: "diesel migration generate".to_string(),
            detail: stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    parse_diesel_paths(&stdout)
}

fn parse_diesel_paths(stdout: &str) -> BlastResult<(PathBuf, PathBuf)> {
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() < 2 {
        return Err(BlastError::Invalid(format!(
            "unexpected diesel output: {}",
            stdout
        )));
    }

    let up_file = lines[0].trim().replace("Creating ", "");
    let down_file = lines[1].trim().replace("Creating ", "");

    Ok((PathBuf::from(up_file), PathBuf::from(down_file)))
}
