use std::path::Path;

use crate::codegen::header;
use crate::error::{BlastError, BlastResult};

const VUE_MARKER_PREFIX: &str = "<!-- AUTO-GENERATED from ";
const VUE_MARKER_FOOTER: &str =
    "<!-- Do not edit by hand. Run `blast gen all` after mutating state. -->";

pub fn vue_marker_for_resource(project_root: &Path, table: &str) -> BlastResult<String> {
    let state_path = header::resource_state_path(project_root, table);
    let relative = state_path.strip_prefix(project_root)?;
    let relative_str = match relative.to_str() {
        Some(s) => s.replace('\\', "/"),
        None => {
            return Err(BlastError::Invalid(format!(
                "non-utf8 state path: {}",
                relative.display()
            )));
        }
    };
    let hash = crate::state::content_hash(&state_path)?;
    Ok(format!(
        "{prefix}{path} @ {hash} -->\n{footer}\n\n",
        prefix = VUE_MARKER_PREFIX,
        path = relative_str,
        hash = hash,
        footer = VUE_MARKER_FOOTER,
    ))
}
