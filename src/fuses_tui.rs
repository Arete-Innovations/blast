use crate::{
    configs::Config,
    error::BlastResult,
    io::traits::{Progress, Sink, SinkExt},
};

pub fn run_with_picker(_config: &Config, sink: &mut dyn Sink, _progress: &mut dyn Progress) -> BlastResult<()> {
    sink.warn("fuses TUI is stubbed pending cursive migration");
    Ok(())
}

pub fn display_fuses_table(_config: &Config) -> BlastResult<()> {
    eprintln!("display_fuses_table is stubbed pending cursive migration");
    Ok(())
}
