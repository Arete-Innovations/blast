use crate::arsenal::scanner::ArsenalReport;
use crate::error::{BlastError, BlastResult};

pub fn serve(_report: ArsenalReport) -> BlastResult<()> {
    Err(BlastError::Invalid(
        "arsenal mcp server not yet implemented".to_string(),
    ))
}
