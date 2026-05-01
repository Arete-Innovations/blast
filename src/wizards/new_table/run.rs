use std::{fs, io, path::Path, time::Duration};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use regex::Regex;

use crate::{
    codegen::enums::scan::scan_project_enums,
    database::write_migration,
    error::{BlastError, BlastResult},
    state::io::save_resource,
};

use super::{
    emit,
    input::{self, Step},
    render,
    state::{ColumnType, Outcome, WizardState},
};

/// Top-level entry: runs the TUI to completion, writes files on commit,
/// returns an outcome the caller can use to chain `migrate` + `gen all`.
pub fn run_picker(project_root: &Path) -> BlastResult<Outcome> {
    let palette = build_type_palette(project_root)?;
    let mut state = WizardState::new(project_root.to_path_buf(), palette);

    enable_raw_mode().map_err(io_to_blast)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(io_to_blast)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(io_to_blast)?;

    let outcome = match event_loop(&mut terminal, &mut state) {
        Ok(o) => Ok(o),
        Err(e) => Err(e),
    };

    disable_raw_mode().map_err(io_to_blast)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture).map_err(io_to_blast)?;
    terminal.show_cursor().map_err(io_to_blast)?;

    outcome
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, state: &mut WizardState) -> BlastResult<Outcome> {
    loop {
        terminal.draw(|f| render::draw(f, state)).map_err(io_to_blast)?;

        let evt_ready = event::poll(Duration::from_millis(200)).map_err(io_to_blast)?;
        if !evt_ready {
            continue;
        }
        let evt = event::read().map_err(io_to_blast)?;

        match input::handle(&evt, state) {
            Step::Stay => {}
            Step::Cancel => {
                return Ok(Outcome {
                    cancelled: true,
                    table_name: state.table_name.value().trim().to_string(),
                    up_sql_path: std::path::PathBuf::new(),
                    down_sql_path: std::path::PathBuf::new(),
                    ron_path: std::path::PathBuf::new(),
                });
            }
            Step::Commit => {
                let outcome = commit(state)?;
                return Ok(outcome);
            }
        }
    }
}

fn commit(state: &WizardState) -> BlastResult<Outcome> {
    let table = emit::validate(state)?;

    let arts = emit::build(state);

    let migration_name = format!("create_{}", table);
    let migration_dir = write_migration(&migration_name, &arts.up_sql, &arts.down_sql)?;
    let up_sql_path = migration_dir.join("up.sql");
    let down_sql_path = migration_dir.join("down.sql");

    let state_dir = state.project_root.join("storage").join("blast").join("state");
    save_resource(&state_dir, &arts.resource)?;
    let ron_path = state_dir.join("resources").join(format!("{}.ron", table));

    Ok(Outcome {
        cancelled: false,
        table_name: table,
        up_sql_path,
        down_sql_path,
        ron_path,
    })
}

/// Build the column-type picker list. Common SQL types come first, then
/// any project-declared enums (from `CREATE TYPE` in migrations) and any
/// existing tables (for FK references).
fn build_type_palette(project_root: &Path) -> BlastResult<Vec<ColumnType>> {
    let mut palette: Vec<ColumnType> = vec![
        ColumnType::Text,
        ColumnType::Varchar(255),
        ColumnType::Integer,
        ColumnType::BigInt,
        ColumnType::Boolean,
        ColumnType::Timestamptz,
        ColumnType::Uuid,
        ColumnType::Jsonb,
        ColumnType::Numeric,
    ];

    let enum_scan = scan_project_enums(project_root)?;
    for parsed in enum_scan.enums {
        palette.push(ColumnType::Enum(parsed.name));
    }

    let tables = scan_project_tables(project_root)?;
    for table in tables {
        palette.push(ColumnType::Fk(table));
    }

    Ok(palette)
}

fn scan_project_tables(project_root: &Path) -> BlastResult<Vec<String>> {
    let migrations_dir = project_root.join("src").join("database").join("migrations");
    if !migrations_dir.is_dir() {
        return Ok(Vec::new());
    }

    let regex = match Regex::new(r"(?im)^\s*CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-z_][a-z0-9_]*)") {
        Ok(r) => r,
        Err(e) => return Err(BlastError::Invalid(format!("scan_project_tables regex: {}", e))),
    };

    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    for entry in fs::read_dir(&migrations_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();

    let mut tables: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for dir in &dirs {
        let up_path = dir.join("up.sql");
        if !up_path.is_file() {
            continue;
        }
        let body = fs::read_to_string(&up_path)?;
        for caps in regex.captures_iter(&body) {
            let name = match caps.get(1) {
                Some(m) => m.as_str().to_string(),
                None => continue,
            };
            tables.insert(name);
        }
    }

    Ok(tables.into_iter().collect())
}

fn io_to_blast(e: io::Error) -> BlastError {
    BlastError::from(e)
}
