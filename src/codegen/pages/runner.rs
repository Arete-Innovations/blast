//! Driver for `blast gen pages` — emits per-resource Vue page SFCs.

use std::path::{Path, PathBuf};

use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::codegen::pages::render::pages_for_resource;
use crate::error::BlastResult;
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state::ResourceState;

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    let resources = ir_loader::load_resource_states(project_root)?;
    emit_for(project_root, &resources, sink, progress)
}

pub fn run_for_resource(
    project_root: &Path,
    resource_name: &str,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    let all = ir_loader::load_resource_states(project_root)?;
    let filtered: Vec<ResourceState> = all
        .into_iter()
        .filter(|r| r.name.as_str() == resource_name)
        .collect();
    if filtered.is_empty() {
        sink.warn(format!("no resource named '{resource_name}' found"));
        return Ok(EmitReport::default());
    }
    emit_for(project_root, &filtered, sink, progress)
}

fn emit_for(
    project_root: &Path,
    resources: &[ResourceState],
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    let step = "crud pages emission";
    progress.step_start(step);

    let mut report = EmitReport::default();
    for r in resources {
        let dir = pages_dir(project_root, r.name.as_str());
        std::fs::create_dir_all(&dir)?;
        let marker = header::marker_for_resource(project_root, r.name.as_str())?;
        for (filename, body) in pages_for_resource(r) {
            let path = dir.join(&filename);
            let full = format!("{marker}{body}");
            std::fs::write(&path, full)?;
            sink.info(format!("wrote {}", path.display()));
            report.written.push(path);
        }
    }

    progress.step_done(step);
    Ok(report)
}

fn pages_dir(project_root: &Path, resource: &str) -> PathBuf {
    project_root
        .join("frontend")
        .join("src")
        .join("pages")
        .join(resource)
}
