use crate::{
    database::{
        migration_wizard::{picker::pick_spec, runner::run, spec::Outcome},
        migrations::ensure_diesel_postgres,
    },
    error::BlastResult,
    io::{
        traits::{Progress, Sink},
        SinkExt,
    },
};

pub fn run_with_picker(sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Option<Outcome>> {
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
