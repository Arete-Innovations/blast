use std::path::Path;

use crate::{error::BlastResult, state, state::ResourceState};

pub fn load_resource_states(project_root: &Path) -> BlastResult<Vec<ResourceState>> {
    let state_dir = project_root.join("storage").join("blast").join("state");
    let names = state::list_resources(&state_dir)?;
    let mut out: Vec<ResourceState> = Vec::with_capacity(names.len());
    for name in &names {
        let resource = state::load_resource(&state_dir, name)?;
        out.push(resource);
    }
    Ok(out)
}
