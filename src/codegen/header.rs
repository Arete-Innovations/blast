use std::path::Path;

pub fn marker_for_state_file(_project_root: &Path, _state_path: &Path) -> Result<String, crate::error::BlastError> {
    Ok(String::new())
}

pub fn marker_for_resource(_project_root: &Path, _table: &str) -> Result<String, crate::error::BlastError> {
    Ok(String::new())
}

pub fn marker_for_app(_project_root: &Path) -> Result<String, crate::error::BlastError> {
    Ok(String::new())
}

pub fn marker_for_schema(_project_root: &Path) -> Result<String, crate::error::BlastError> {
    Ok(String::new())
}
