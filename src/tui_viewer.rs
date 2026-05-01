use std::io;

use crate::configs::Config;

pub fn run_tui_log_viewer(_level: &str, _config: &Config) -> io::Result<()> {
    eprintln!("tui_viewer is stubbed pending cursive migration");
    Ok(())
}
