use crate::error::BlastResult;
use crate::logger;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputMode {
    LogFile,
}

pub fn log(message: &str) -> BlastResult<()> {
    logger::info(message)
}

pub fn set_output_mode(mode: OutputMode) {
    match mode {
        OutputMode::LogFile => {
            if let Err(e) = logger::init(logger::RuntimeMode::Dashboard, None) {
                eprintln!("logger init failed: {}", e);
            }
        }
    }
}

pub fn set_quiet_mode(quiet: bool) {
    logger::set_quiet_mode(quiet);
}

pub fn set_log_file_path(path: &std::path::Path) -> BlastResult<()> {
    logger::init(logger::RuntimeMode::Dashboard, Some(path))
}
