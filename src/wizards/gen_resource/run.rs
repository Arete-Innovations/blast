use std::{collections::BTreeSet, path::PathBuf};

use dialoguer::{theme::ColorfulTheme, Confirm};

use crate::{
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    schema_parser::{self, ParsedTable},
    state::{
        self, io as state_io,
        names::{FieldName, ResourceName, SqlType},
        resource::{FieldState, ResourceState},
    },
    wizards::gen_resource::{confirm, fields, gen_level, list, pick, schema_diff, validators, verbs, ws},
};

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

pub fn pick_args_with_name(project_root: PathBuf, name: Option<String>) -> BlastResult<Args> {
    Ok(Args { project_root, resource_name: name })
}

pub fn run(args: Args, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    let state_dir = args.project_root.join("storage").join("blast").join("state");
    let schema_path = args.project_root.join("src").join("database").join("schema.rs");

    if !schema_path.is_file() {
        return Err(BlastError::NotFound(format!("schema.rs not found at {}", schema_path.display())));
    }

    let tables = schema_parser::parse_schema(&schema_path)?;
    if tables.is_empty() {
        return Err(BlastError::Invalid(format!("no tables found in {}", schema_path.display())));
    }

    progress.step_start("pick table");
    let table = pick::select_table(&tables, args.resource_name.as_deref())?;
    progress.step_done(&format!("table: {}", table.name));

    let resource_name = ResourceName::new(table.name.clone());
    let existing = load_existing_or_none(&state_dir, &resource_name)?;
    let was_existing = existing.is_some();

    if was_existing {
        sink.info(format!("editing existing resource state for `{}`", resource_name));
    } else {
        sink.info(format!("authoring new resource state for `{}`", resource_name));
    }

    let mut resource = match existing {
        Some(prev) => prev,
        None => ResourceState::new(resource_name.clone()),
    };

    if was_existing {
        progress.step_start("schema diff");
        detect_and_apply_drift(table, &mut resource, sink)?;
        progress.step_done("schema diff");
    }

    progress.step_start("fields");
    fields::collect_fields(table, &mut resource)?;
    progress.step_done("fields");

    progress.step_start("validators");
    validators::collect_validators(&mut resource)?;
    progress.step_done("validators");

    progress.step_start("verbs");
    verbs::collect_verbs(table, &mut resource)?;
    progress.step_done("verbs");

    progress.step_start("list options");
    list::collect_list_options(table, &mut resource)?;
    progress.step_done("list options");

    progress.step_start("ws events");
    ws::collect_ws_events(table, &mut resource)?;
    progress.step_done("ws events");

    progress.step_start("gen level");
    gen_level::collect_gen_level(&mut resource)?;
    progress.step_done("gen level");

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

    let action = if was_existing { WriteAction::Updated } else { WriteAction::Created };
    let state_file = state_io::resource_path(&state_dir, &resource.name);
    sink.success(format!("wrote {}", state_file.display()));
    Ok(Outcome { state_file, action })
}

fn load_existing_or_none(state_dir: &std::path::Path, name: &ResourceName) -> BlastResult<Option<ResourceState>> {
    let path = state_io::resource_path(state_dir, name);
    if !path.is_file() {
        return Ok(None);
    }
    let loaded = state::load_resource(state_dir, name)?;
    Ok(Some(loaded))
}

fn detect_and_apply_drift(table: &ParsedTable, resource: &mut ResourceState, sink: &mut dyn Sink) -> BlastResult<()> {
    let schema_columns: Vec<(String, String)> = table.columns.iter().map(|c| (c.name.clone(), c.diesel_type.clone())).collect();

    let diff = schema_diff::compute(&schema_columns, resource);
    if schema_diff::is_empty(&diff) {
        return Ok(());
    }

    sink.warn(schema_diff::render(&diff));

    if diff.added_columns.is_empty() {
        return Ok(());
    }

    let prompt = format!(
        "Schema has changed: {} added, {} removed, {} type-changed. Apply additions automatically with default variants?",
        diff.added_columns.len(),
        diff.removed_columns.len(),
        diff.type_changes.len(),
    );

    let apply = Confirm::with_theme(&ColorfulTheme::default()).with_prompt(prompt).default(true).interact()?;

    if !apply {
        return Ok(());
    }

    apply_added_columns(table, &diff.added_columns, resource);
    Ok(())
}

fn apply_added_columns(table: &ParsedTable, added: &[(FieldName, SqlType)], resource: &mut ResourceState) {
    for (field_name, sql_type) in added {
        let column_match = table.columns.iter().find(|col| col.name == field_name.as_str());
        let Some(column) = column_match else {
            continue;
        };
        let is_pk = table.primary_key.iter().any(|pk| pk == &column.name);
        let variants: BTreeSet<_> = fields::smart_defaults(&column.name, is_pk);
        resource.fields.insert(
            field_name.clone(),
            FieldState {
                sql_type: sql_type.clone(),
                variants,
                nullable: column.nullable,
                primary_key: is_pk,
                validators: BTreeSet::new(),
            },
        );
    }
}
