// This is a stub file that redirects to the new logger module.
// It exists only for backward compatibility during the transition.

use crate::logger;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputMode {
    #[allow(dead_code)]
    Stdout, // Print to stdout only
    LogFile, // Print to log file only
    #[allow(dead_code)]
    StdoutAndLogFile, // Print to both stdout and log file
}

// Redirect to logger::log
pub fn log(message: &str) -> std::io::Result<()> {
    logger::info(message).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

// Redirect to logger::log_raw (which is now logger::info)
#[allow(dead_code)]
pub fn log_raw(message: &str) -> std::io::Result<()> {
    logger::info(message).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

// Set output mode globally (redirects to logger::set_runtime_mode)
pub fn set_output_mode(mode: OutputMode) {
    match mode {
        OutputMode::Stdout => crate::logger::init(crate::logger::RuntimeMode::Cli, None).unwrap_or_default(),
        OutputMode::LogFile => crate::logger::init(crate::logger::RuntimeMode::Dashboard, None).unwrap_or_default(),
        OutputMode::StdoutAndLogFile => crate::logger::init(crate::logger::RuntimeMode::Cli, None).unwrap_or_default(),
    }
}

// Set operation context (no longer needed)
#[allow(dead_code)]
pub fn set_operation_context(_context: &str) {
    // This is a no-op in the new system
}

// Set quiet mode
pub fn set_quiet_mode(quiet: bool) {
    logger::set_quiet_mode(quiet);
}

// Stub for DashboardProgress that uses the new Progress struct
#[allow(dead_code)]
pub struct DashboardProgress {
    progress: logger::Progress,
}

#[allow(dead_code)]
impl DashboardProgress {
    pub fn new_bar(total: u64) -> Self {
        Self {
            progress: logger::create_progress(Some(total)),
        }
    }

    pub fn new_spinner() -> Self {
        Self { progress: logger::create_progress(None) }
    }

    pub fn set_message(&self, message: &str) {
        let mut progress = self.progress.clone();
        progress.set_message(message);
    }

    pub fn inc(&self, delta: u64) {
        let mut progress = self.progress.clone();
        progress.inc(delta);
    }

    pub fn finish_with_message(&self, message: &str) {
        let mut progress = self.progress.clone();
        progress.success(message);
    }

    pub fn render(&self) {
        // No-op in the new system
    }
}

// These functions are no longer needed but included for backward compatibility
#[allow(dead_code)]
pub fn get_output_mode() -> OutputMode {
    OutputMode::Stdout // Default value
}

#[allow(dead_code)]
pub fn get_operation_context() -> String {
    "CLI".to_string() // Default value
}

#[allow(dead_code)]
pub fn is_quiet_mode() -> bool {
    false // Default value
}

pub fn set_log_file_path(path: &std::path::Path) -> std::io::Result<()> {
    logger::init(logger::RuntimeMode::Dashboard, Some(path)).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

#[allow(dead_code)]
pub fn set_operations_log_path(_path: &std::path::Path) {
    // This is a no-op in the new system
}

#[allow(dead_code)]
pub fn set_progress_log_path(_path: &std::path::Path) {
    // This is a no-op in the new system
}
