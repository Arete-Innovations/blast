use std::path::{Path, PathBuf};

use crate::{
    error::BlastResult,
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "leptos forms generation";

pub fn run(_project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);
    sink.info(format!("{STEP_LABEL}: stub — leptos-form derive emitter pending phase 4 follow-up"));
    progress.step_done(STEP_LABEL);
    Ok(EmitReport::default())
}
