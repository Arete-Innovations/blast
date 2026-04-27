use diesel::pg::PgConnection;
use diesel_migrations::{EmbeddedMigrations, HarnessWithOutput, MigrationHarness};

use crate::{cata_log, meltdown::*};

pub fn run_pending(conn: &mut PgConnection, migrations: EmbeddedMigrations) -> Result<(), MeltDown> {
    let mut harness = HarnessWithOutput::write_to_stdout(conn);
    let applied = harness.run_pending_migrations(migrations).map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("migration failed: {}", e)))?;

    if applied.is_empty() {
        cata_log!(Info, "no pending migrations; schema is up-to-date");
    } else {
        cata_log!(Info, format!("running {} pending migration(s)", applied.len()));
        for version in &applied {
            cata_log!(Info, format!("migration applied: {}", version));
        }
    }

    Ok(())
}
