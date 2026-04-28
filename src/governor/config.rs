use std::path::Path;

use crate::{
    error::{BlastError, BlastResult},
    state::{AppPolicySection, FeLintState},
};

pub fn load_or_default(project_root: &Path) -> BlastResult<FeLintState> {
    let state_dir = project_root.join("storage/blast/state");
    let app_state = match crate::state::load_app(&state_dir) {
        Ok(s) => s,
        Err(BlastError::Io(io_err)) if io_err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FeLintState::default());
        }
        Err(e) => return Err(e),
    };
    match app_state.sections.get("fe_lint") {
        Some(AppPolicySection::FeLint(state)) => Ok(state.clone()),
        Some(_non_fe_lint) => Ok(FeLintState::default()),
        None => Ok(FeLintState::default()),
    }
}
