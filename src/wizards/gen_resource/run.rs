use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::schema_parser::{self, ParsedTable};
use crate::state::names::ResourceName;
use crate::state::resource::ResourceState;
use crate::state::{self, io as state_io};
use crate::wizards::gen_resource::{confirm, fields, list, pick, verbs, ws};
use std::path::PathBuf;

pub struct Args {
    pub project_root: PathBuf,
    pub resource_name: Option<String>,
}

pub struct Outcome {
    pub state_file: PathBuf,
    pub action: WriteAction,
}

pub enum WriteAction {
    Created,
    Updated,
    Cancelled,
}

pub fn pick_args(project_root: PathBuf) -> BlastResult<Args> {
    pick_args_with_name(project_root, None)
}

pub fn pick_args_with_name(project_root: PathBuf, name: Option<String>) -> BlastResult<Args> {
    Ok(Args {
        project_root,
        resource_name: name,
    })
}

pub fn run(
    args: Args,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<Outcome> {
    let state_dir = args.project_root.join("storage").join("blast").join("state");
    let schema_path = args.project_root.join("src").join("database").join("schema.rs");

    if !schema_path.is_file() {
        return Err(BlastError::NotFound(format!(
            "schema.rs not found at {}",
            schema_path.display()
        )));
    }

    let tables = schema_parser::parse_schema(&schema_path)?;
    if tables.is_empty() {
        return Err(BlastError::Invalid(format!(
            "no tables found in {}",
            schema_path.display()
        )));
    }

    progress.step_start("pick table");
    let table = pick::select_table(&tables, args.resource_name.as_deref())?;
    progress.step_done(&format!("table: {}", table.name));

    let resource_name = ResourceName::new(table.name.clone());
    let existing = load_existing_or_none(&state_dir, &resource_name)?;
    let was_existing = existing.is_some();

    if was_existing {
        sink.info(format!(
            "editing existing resource state for `{}`",
            resource_name
        ));
    } else {
        sink.info(format!(
            "authoring new resource state for `{}`",
            resource_name
        ));
    }

    let mut resource = match existing {
        Some(prev) => prev,
        None => ResourceState::new(resource_name.clone()),
    };

    progress.step_start("fields");
    fields::collect_fields(table, &mut resource)?;
    progress.step_done("fields");

    progress.step_start("verbs");
    verbs::collect_verbs(table, &mut resource)?;
    progress.step_done("verbs");

    progress.step_start("list options");
    list::collect_list_options(table, &mut resource)?;
    progress.step_done("list options");

    progress.step_start("ws events");
    ws::collect_ws_events(table, &mut resource)?;
    progress.step_done("ws events");

    progress.step_start("confirm");
    let confirmed = confirm::review_and_confirm(&resource, sink)?;
    if !confirmed {
        progress.step_done("confirm: cancelled");
        sink.warn("wizard cancelled — no state file written");
        return Ok(Outcome {
            state_file: state_io::resource_path(&state_dir, &resource.name),
            action: WriteAction::Cancelled,
        });
    }
    progress.step_done("confirm");

    progress.step_start("write state file");
    state::save_resource(&state_dir, &resource)?;
    progress.step_done("write state file");

    let action = if was_existing {
        WriteAction::Updated
    } else {
        WriteAction::Created
    };
    let state_file = state_io::resource_path(&state_dir, &resource.name);
    sink.success(format!("wrote {}", state_file.display()));
    Ok(Outcome { state_file, action })
}

fn load_existing_or_none(
    state_dir: &std::path::Path,
    name: &ResourceName,
) -> BlastResult<Option<ResourceState>> {
    let path = state_io::resource_path(state_dir, name);
    if !path.is_file() {
        return Ok(None);
    }
    let loaded = state::load_resource(state_dir, name)?;
    Ok(Some(loaded))
}
