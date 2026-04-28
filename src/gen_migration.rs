use std::{env, process::Command};

use crate::{database::write_migration, error::BlastResult, logger};

const UP_SKELETON: &str = "-- Write your custom up migration here.\n";
const DOWN_SKELETON: &str = "-- Write your custom down migration here.\n";

pub fn run_custom(name: &str) -> BlastResult<()> {
    let dir = write_migration(name, UP_SKELETON, DOWN_SKELETON)?;
    let up_path = dir.join("up.sql");
    let down_path = dir.join("down.sql");

    logger::success(&format!("Migration scaffold created: {}", dir.display()))?;
    logger::info(&format!("  up:   {}", up_path.display()))?;
    logger::info(&format!("  down: {}", down_path.display()))?;

    let editor = match env::var("EDITOR") {
        Ok(v) => v,
        Err(e) => {
            drop(e);
            logger::warning("$EDITOR is not set; edit the up.sql / down.sql files manually with your editor of choice.")?;
            return Ok(());
        }
    };

    if editor.trim().is_empty() {
        logger::warning("$EDITOR is empty; edit the up.sql / down.sql files manually with your editor of choice.")?;
        return Ok(());
    }

    logger::info(&format!("Opening {} in {}...", up_path.display(), editor))?;
    let status = Command::new(&editor).arg(&up_path).status()?;
    if !status.success() {
        logger::warning(&format!("{} exited with status {:?}; migration files were still written.", editor, status))?;
    }

    Ok(())
}
