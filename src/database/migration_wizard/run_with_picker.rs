use crate::database::migration_wizard::picker::pick_spec;
use crate::database::migration_wizard::runner::run;
use crate::database::migration_wizard::spec::Outcome;
use crate::database::migrations::ensure_diesel_postgres;
use crate::error::BlastResult;
use crate::io::traits::{Progress, Sink};
use crate::io::SinkExt;

pub fn run_with_picker(
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<Option<Outcome>> {
    ensure_diesel_postgres();

    let spec = pick_spec()?;
    match spec {
        None => {
            sink.info("migration creation cancelled");
            Ok(None)
        }
        Some(resolved) => {
            let outcome = run(resolved, sink, progress)?;
            Ok(Some(outcome))
        }
    }
}
