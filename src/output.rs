use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;

// Define output modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputMode {
    Stdout,           // Print to stdout only (for CLI one-shot commands)
    LogFile,          // Print to log file only (for dashboard interactive mode)
    StdoutAndLogFile, // Print to both stdout and log file
}

// Global output mode
lazy_static! {
    static ref CURRENT_MODE: Arc<Mutex<OutputMode>> = Arc::new(Mutex::new(OutputMode::Stdout));
    static ref LOG_FILE_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    static ref OPERATION_CONTEXT: Arc<Mutex<String>> = Arc::new(Mutex::new(String::from("blast")));
    static ref QUIET_MODE: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref OPERATIONS_LOG_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    pub static ref PROGRESS_LOG_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
}

// Set output mode globally
pub fn set_output_mode(mode: OutputMode) {
    let mut current_mode = CURRENT_MODE.lock().unwrap();
    *current_mode = mode;
}

// Set log file path
pub fn set_log_file_path(path: &Path) -> std::io::Result<()> {
    // Create directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create or append to the log file (never truncate)
    let mut file = OpenOptions::new().create(true).write(true).append(true).open(path)?;

    // Write a session separator to the file
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "\n\n--- New Blast Session: {} ---", timestamp)?;
    writeln!(file, "-----------------------")?;

    // Close the file (it will be reopened as needed)
    drop(file);

    let mut log_path = LOG_FILE_PATH.lock().unwrap();
    *log_path = Some(path.to_path_buf());

    // Also ensure the operations log exists in the same directory
    if let Some(parent) = path.parent() {
        let operations_log_path = parent.join("operations.log");
        if !operations_log_path.exists() {
            let mut ops_file = OpenOptions::new().create(true).write(true).open(&operations_log_path)?;

            writeln!(ops_file, "--- Blast Operations Log ---")?;
            writeln!(ops_file, "Started at: {}", timestamp)?;
            writeln!(ops_file, "---------------------------")?;
        }
    }

    Ok(())
}

// Get current output mode
pub fn get_output_mode() -> OutputMode {
    let mode = CURRENT_MODE.lock().unwrap();
    *mode
}

// Set operation context (used for logging)
pub fn set_operation_context(context: &str) {
    let mut op_context = OPERATION_CONTEXT.lock().unwrap();
    *op_context = context.to_string();
}

// Get operation context
pub fn get_operation_context() -> String {
    let op_context = OPERATION_CONTEXT.lock().unwrap();
    op_context.clone()
}

// Set quiet mode - when true, no output will go to stdout regardless of output mode
// This is particularly useful for interactive mode where we want to suppress all output
pub fn set_quiet_mode(quiet: bool) {
    let mut quiet_mode = QUIET_MODE.lock().unwrap();
    *quiet_mode = quiet;
}

// Check if quiet mode is enabled
pub fn is_quiet_mode() -> bool {
    let quiet_mode = QUIET_MODE.lock().unwrap();
    *quiet_mode
}

// Set operations log path
pub fn set_operations_log_path(path: &Path) {
    let mut ops_path = OPERATIONS_LOG_PATH.lock().unwrap();
    *ops_path = Some(path.to_path_buf());
}

// Set progress log path
pub fn set_progress_log_path(path: &Path) {
    let mut prog_path = PROGRESS_LOG_PATH.lock().unwrap();
    *prog_path = Some(path.to_path_buf());
}

// Append to log file with current timestamp and context
pub fn log(message: &str) -> std::io::Result<()> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let context = get_operation_context();
    let formatted_message = format!("[{}] [{}] {}", timestamp, context, message);

    match get_output_mode() {
        OutputMode::Stdout => {
            // Only print to stdout if we're not in quiet mode
            if !is_quiet_mode() {
                println!("{}", message);
            }
            Ok(())
        }
        OutputMode::LogFile => {
            // In log file mode, only write to the single log file
            log_to_file(&formatted_message)
        }
        OutputMode::StdoutAndLogFile => {
            // Only print to stdout if we're not in quiet mode
            if !is_quiet_mode() {
                println!("{}", message);
            }
            // Write to the single log file
            log_to_file(&formatted_message)
        }
    }
}

// Log a message without timestamp or context formatting
// Now just appends to the log file for progress updates
pub fn log_raw(message: &str) -> std::io::Result<()> {
    match get_output_mode() {
        OutputMode::Stdout => {
            // Only print to stdout if we're not in quiet mode
            if !is_quiet_mode() {
                println!("{}", message);
            }
            Ok(())
        }
        OutputMode::LogFile => {
            // In interactive mode, append to the single log file
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let formatted_message = format!("[{}] {}", timestamp, message);
            log_to_file(&formatted_message)
        }
        OutputMode::StdoutAndLogFile => {
            // Only print to stdout if we're not in quiet mode
            if !is_quiet_mode() {
                println!("{}", message);
            }
            // Also log to file with timestamp
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let formatted_message = format!("[{}] {}", timestamp, message);
            log_to_file(&formatted_message)
        }
    }
}

// Helper function to write to the single log file
fn log_to_file(message: &str) -> std::io::Result<()> {
    let log_path_guard = LOG_FILE_PATH.lock().unwrap();
    if let Some(log_path) = &*log_path_guard {
        let mut file = OpenOptions::new().create(true).write(true).append(true).open(log_path)?;
        writeln!(file, "{}", message)?;
    } else {
        // Fallback to stdout if no log file is set
        println!("Warning: No log file set, falling back to stdout");
        println!("{}", message);
    }
    Ok(())
}

// Custom progress renderer for dashboard mode
#[derive(Clone)]
pub struct DashboardProgress {
    pub message: String,
    pub current: u64,
    pub total: u64,
    pub is_spinner: bool,
}

impl DashboardProgress {
    pub fn new_bar(total: u64) -> Self {
        Self {
            message: String::new(),
            current: 0,
            total,
            is_spinner: false,
        }
    }

    pub fn new_spinner() -> Self {
        Self {
            message: String::new(),
            current: 0,
            total: 0,
            is_spinner: true,
        }
    }

    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.render();
    }

    pub fn inc(&mut self, delta: u64) {
        self.current += delta;
        if self.current > self.total {
            self.current = self.total;
        }
        self.render();
    }

    pub fn finish_with_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.current = self.total;
        self.render();

        // Add a newline after completion
        if get_output_mode() == OutputMode::LogFile {
            let _ = log("");
        }
    }

    pub fn render(&self) {
        let mode = get_output_mode();

        // When in LogFile mode, NEVER render to stdout
        // Render to stdout only if explicitly set to Stdout or StdoutAndLogFile
        if mode == OutputMode::Stdout || mode == OutputMode::StdoutAndLogFile {
            if self.is_spinner {
                print!("\r⟳ {} ", self.message);
            } else {
                let width = 30;
                let percent = if self.total > 0 { self.current as f64 / self.total as f64 } else { 0.0 };
                let filled = (width as f64 * percent) as usize;
                let empty = width - filled;

                print!("\r[{}>{}] {}/{} {}", "#".repeat(filled), " ".repeat(empty), self.current, self.total, self.message);
            }
            std::io::stdout().flush().unwrap();
        }

        // Render to log file if needed
        if mode == OutputMode::LogFile || mode == OutputMode::StdoutAndLogFile {
            // Use log_raw which now handles truncation internally
            if self.is_spinner {
                let _ = log_raw(&format!("⟳ {}", self.message));
            } else {
                let width = 30;
                let percent = if self.total > 0 { self.current as f64 / self.total as f64 } else { 0.0 };
                let filled = (width as f64 * percent) as usize;
                let empty = width - filled;

                let _ = log_raw(&format!("[{}>{}] {}/{} {}", "#".repeat(filled), " ".repeat(empty), self.current, self.total, self.message));
            }
        }
    }
}
