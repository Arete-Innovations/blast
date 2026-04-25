use crate::codegen::ir_loader;
use crate::error::BlastResult;
use std::path::Path;

/// Top-level entry for the frontend codegen pass. Reads primer IR from
/// `target/primer/*.json` and emits TS validators + list query helpers under
/// `frontend/src/generated/`.
///
/// Stage 1 — skeleton only; subsequent commits fill in the writers.
pub fn run_frontend(project_root: &Path) -> BlastResult<()> {
    let _ir = ir_loader::load_primer_ir(project_root)?;
    Ok(())
}
