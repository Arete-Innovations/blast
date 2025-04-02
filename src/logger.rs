use crate::configs::Config;
use chrono::Local;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use std::env;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// Runtime mode enum for determining where output should go
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Cli,       // Standard CLI mode - print to stdout with colors
    Dashboard, // Dashboard mode - log to file only
}

// Log level for message categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Success,
}

// Global state - minimized to just what we need
lazy_static! {
    static ref RUNTIME_MODE: Arc<Mutex<RuntimeMode>> = Arc::new(Mutex::new(RuntimeMode::Cli));
    static ref LOG_FILE_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    static ref QUIET_MODE: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref VERBOSE_MODE: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
}

// Standard log files that should exist in each project
pub const STANDARD_LOG_FILES: [&str; 5] = ["server.log", "error.log", "info.log", "debug.log", "warning.log"];

// Initialize the logging system
pub fn init(mode: RuntimeMode, log_path: Option<&Path>) -> Result<(), Box<dyn Error>> {
    // Set runtime mode
    let mut current_mode = RUNTIME_MODE.lock().unwrap();
    *current_mode = mode;

    // If log path provided, initialize log file
    if let Some(path) = log_path {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open log file
        let mut file = OpenOptions::new().create(true).write(true).append(true).open(path)?;

        // Write session header
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        writeln!(file, "\n--- New Blast Session: {} ---", timestamp)?;
        writeln!(file, "-------------------------")?;

        // Update global log path
        let mut log_path_guard = LOG_FILE_PATH.lock().unwrap();
        *log_path_guard = Some(path.to_path_buf());
    }

    Ok(())
}

// Determine if running in interactive mode
#[allow(dead_code)]
pub fn is_interactive_mode() -> bool {
    env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1"
}

// Set quiet mode (suppress stdout even in CLI mode)
pub fn set_quiet_mode(quiet: bool) {
    let mut quiet_mode = QUIET_MODE.lock().unwrap();
    *quiet_mode = quiet;
}

// Set verbose mode (show detailed logging)
pub fn set_verbose_mode(verbose: bool) {
    let mut verbose_mode = VERBOSE_MODE.lock().unwrap();
    *verbose_mode = verbose;
}

// Get current quiet mode
fn is_quiet() -> bool {
    let quiet_mode = QUIET_MODE.lock().unwrap();
    *quiet_mode
}

// Get current verbose mode
fn is_verbose() -> bool {
    // Check for both VERBOSE_MODE flag and BLAST_VERBOSE environment variable
    let verbose_mode = VERBOSE_MODE.lock().unwrap();
    *verbose_mode || env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1"
}

// Get current runtime mode
fn get_mode() -> RuntimeMode {
    let mode = RUNTIME_MODE.lock().unwrap();
    *mode
}

// Log to file and/or stdout depending on mode
pub fn log(level: LogLevel, message: &str) -> Result<(), Box<dyn Error>> {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

    // Format message with emoji for visual recognition
    let _emoji = match level {
        LogLevel::Debug => "🔍",
        LogLevel::Info => "ℹ️",
        LogLevel::Warning => "⚠️",
        LogLevel::Error => "❌",
        LogLevel::Success => "✅",
    };

    // Format message for log file (no colors, with timestamp)
    let log_message = format!(
        "[{}] {}  {}",
        timestamp,
        match level {
            LogLevel::Debug => "[DEBUG]",
            LogLevel::Info => "[INFO]",
            LogLevel::Warning => "[WARNING]",
            LogLevel::Error => "[ERROR]",
            LogLevel::Success => "[SUCCESS]",
        },
        message
    );

    // Check if we should show this message based on verbosity settings
    // Always show errors, warnings, and successes regardless of verbosity
    let should_show_stdout = match level {
        LogLevel::Debug => is_verbose(),
        LogLevel::Info => is_verbose(),
        LogLevel::Warning => true,
        LogLevel::Error => true,
        LogLevel::Success => true,
    };

    // Format message for stdout (with colors) if we should show it
    let stdout_message = if should_show_stdout {
        format!(
            "{} {} {}",
            style(format!("[{}]", timestamp)).dim(),
            match level {
                LogLevel::Debug => style("[DEBUG]").dim(),
                LogLevel::Info => style("[INFO]").cyan(),
                LogLevel::Warning => style("[WARNING]").yellow(),
                LogLevel::Error => style("[ERROR]").red(),
                LogLevel::Success => style("[SUCCESS]").green(),
            },
            message
        )
    } else {
        String::new() // Empty string if we shouldn't show it
    };

    // Always write to log file
    let log_path_guard = LOG_FILE_PATH.lock().unwrap();
    if let Some(log_path) = &*log_path_guard {
        let mut file = OpenOptions::new().create(true).write(true).append(true).open(log_path)?;
        writeln!(file, "{}", log_message)?;
    }

    // Only print to stdout if in CLI mode and not quiet and we should show this level
    if get_mode() == RuntimeMode::Cli && !is_quiet() && should_show_stdout && !stdout_message.is_empty() {
        println!("{}", stdout_message);
    }

    Ok(())
}

// Helper functions for specific log levels
pub fn debug(message: &str) -> Result<(), Box<dyn Error>> {
    log(LogLevel::Debug, message)
}

pub fn info(message: &str) -> Result<(), Box<dyn Error>> {
    log(LogLevel::Info, message)
}

pub fn warning(message: &str) -> Result<(), Box<dyn Error>> {
    log(LogLevel::Warning, message)
}

pub fn error(message: &str) -> Result<(), Box<dyn Error>> {
    log(LogLevel::Error, message)
}

pub fn success(message: &str) -> Result<(), Box<dyn Error>> {
    log(LogLevel::Success, message)
}

// Create a progress bar that works in both CLI and dashboard modes
pub fn create_progress(steps: Option<u64>) -> Progress {
    Progress::new(steps)
}

// Progress tracker that works in both CLI and dashboard modes
#[derive(Clone)]
pub struct Progress {
    bar: ProgressBar,
    spinner_char: char,
    last_message: String,
    total: Option<u64>,
    current: u64,
}

impl Progress {
    fn new(steps: Option<u64>) -> Self {
        let bar = match steps {
            Some(total) => {
                let pb = ProgressBar::new(total);
                let style = ProgressStyle::default_bar().template("[{bar:40.cyan/blue}] {pos}/{len} {msg}").unwrap().progress_chars("#>-");
                pb.set_style(style);
                pb
            }
            None => {
                let pb = ProgressBar::new_spinner();
                let style = ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap();
                pb.set_style(style);
                pb.enable_steady_tick(std::time::Duration::from_millis(120));
                pb
            }
        };

        Progress {
            bar,
            spinner_char: '⟳',
            last_message: String::new(),
            total: steps,
            current: 0,
        }
    }

    pub fn set_message(&mut self, msg: &str) -> &mut Self {
        self.last_message = msg.to_string();

        match get_mode() {
            RuntimeMode::Cli => {
                if !is_quiet() {
                    self.bar.set_message(msg.to_string());
                }
            }
            RuntimeMode::Dashboard => {
                // Log progress update to file
                let log_path_guard = LOG_FILE_PATH.lock().unwrap();
                if let Some(log_path) = &*log_path_guard {
                    if let Ok(mut file) = OpenOptions::new().create(true).write(true).append(true).open(log_path) {
                        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

                        if let Some(total) = self.total {
                            // Progress bar format for file
                            let _ = writeln!(file, "[{}] [PROGRESS] [{}/{}] {}", timestamp, self.current, total, msg);
                        } else {
                            // Spinner format for file
                            let _ = writeln!(file, "[{}] [PROGRESS] {} {}", timestamp, self.spinner_char, msg);

                            // Rotate spinner character
                            self.spinner_char = match self.spinner_char {
                                '⟳' => '⟲',
                                '⟲' => '↻',
                                '↻' => '↺',
                                _ => '⟳',
                            };
                        }
                    }
                }
            }
        }

        self
    }

    #[allow(dead_code)]
    pub fn inc(&mut self, delta: u64) -> &mut Self {
        self.current += delta;
        if let Some(total) = self.total {
            if self.current > total {
                self.current = total;
            }
        }

        match get_mode() {
            RuntimeMode::Cli => {
                if !is_quiet() {
                    self.bar.inc(delta);
                }
            }
            RuntimeMode::Dashboard => {
                // Update progress in file
                let current_msg = self.last_message.clone();
                self.set_message(&current_msg);
            }
        }

        self
    }

    pub fn success(&mut self, msg: &str) {
        let formatted_msg = format!("✅ {}", msg);

        match get_mode() {
            RuntimeMode::Cli => {
                if !is_quiet() {
                    self.bar.finish_with_message(formatted_msg);
                }
            }
            RuntimeMode::Dashboard => {
                // Log success to file
                let _ = success(msg);
            }
        }
    }

    pub fn error(&mut self, msg: &str) {
        let formatted_msg = format!("❌ {}", msg);

        match get_mode() {
            RuntimeMode::Cli => {
                if !is_quiet() {
                    self.bar.finish_with_message(formatted_msg);
                }
            }
            RuntimeMode::Dashboard => {
                // Log error to file
                let _ = error(msg);
            }
        }
    }

    pub fn warning(&mut self, msg: &str) -> Result<(), Box<dyn Error>> {
        let formatted_msg = format!("⚠️ {}", msg);

        match get_mode() {
            RuntimeMode::Cli => {
                if !is_quiet() {
                    self.bar.set_message(formatted_msg);
                }
            }
            RuntimeMode::Dashboard => {
                // Log warning to file
                warning(msg)?;
            }
        }

        Ok(())
    }
}

// Get all log files in the logs directory
pub fn get_log_files(config: &Config) -> Vec<PathBuf> {
    let logs_dir = config.project_dir.join("storage").join("logs");

    // Create logs directory if it doesn't exist
    if !logs_dir.exists() {
        if let Err(e) = fs::create_dir_all(&logs_dir) {
            let _ = error(&format!("Error creating logs directory: {}", e));
            return Vec::new();
        }
    }

    // Find all log files
    let mut log_files = Vec::new();

    // Read the directory and filter for log files
    if let Ok(entries) = fs::read_dir(&logs_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "log") {
                log_files.push(path);
            }
        }
    }

    // Also include blast.log if it exists
    let blast_log = config.project_dir.join("storage").join("blast").join("blast.log");
    if blast_log.exists() {
        log_files.push(blast_log);
    }

    log_files
}

// Truncate a specific log file
pub fn truncate_log_file(log_path: &Path) -> Result<(), Box<dyn Error>> {
    let mut progress = create_progress(None);
    progress.set_message(&format!("Truncating log file: {}", log_path.display()));

    // Open the file in truncate mode
    let mut file = File::create(log_path)?;

    // Write a header to the file
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "--- Log file truncated at {} ---", timestamp)?;

    progress.success(&format!("Truncated log file: {}", log_path.display()));
    Ok(())
}

// Truncate all log files in the logs directory
pub fn truncate_all_logs(config: &Config) -> Result<(), Box<dyn Error>> {
    let log_files = get_log_files(config);

    if log_files.is_empty() {
        info("No log files found")?;
        return Ok(());
    }

    for log_path in log_files {
        if let Err(e) = truncate_log_file(&log_path) {
            error(&format!("Error truncating {}: {}", log_path.display(), e))?;
        }
    }

    Ok(())
}

// Truncate a specific log file by name
pub fn truncate_specific_log(config: &Config, file_name: Option<String>) -> Result<(), Box<dyn Error>> {
    // If no specific file was provided, truncate all logs
    if file_name.is_none() {
        return truncate_all_logs(config);
    }

    let file_name = file_name.unwrap();

    // Try to find the exact file
    let logs_dir = config.project_dir.join("storage").join("logs");
    let specific_path = logs_dir.join(&file_name);

    // Also check in the blast directory
    let blast_dir = config.project_dir.join("storage").join("blast");
    let blast_specific_path = blast_dir.join(&file_name);

    if specific_path.exists() {
        return truncate_log_file(&specific_path);
    } else if blast_specific_path.exists() {
        return truncate_log_file(&blast_specific_path);
    } else {
        // Try with .log extension if not provided
        let with_extension = if file_name.ends_with(".log") { file_name.clone() } else { format!("{}.log", file_name) };

        let specific_path_with_ext = logs_dir.join(&with_extension);
        let blast_specific_path_with_ext = blast_dir.join(&with_extension);

        if specific_path_with_ext.exists() {
            return truncate_log_file(&specific_path_with_ext);
        } else if blast_specific_path_with_ext.exists() {
            return truncate_log_file(&blast_specific_path_with_ext);
        }

        // If still not found, show an error
        return Err(format!("Log file not found: {}", file_name).into());
    }
}

// Create standard log files if they don't exist
pub fn ensure_log_files_exist(config: &Config) -> Result<(), Box<dyn Error>> {
    let logs_dir = config.project_dir.join("storage").join("logs");

    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir)?;

    // Also create blast directory
    let blast_dir = config.project_dir.join("storage").join("blast");
    fs::create_dir_all(&blast_dir)?;

    // Create standard log files if they don't exist
    for log_file in STANDARD_LOG_FILES.iter() {
        let log_path = logs_dir.join(log_file);
        if !log_path.exists() {
            let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(&log_path)?;

            writeln!(file, "--- Log file initialized: {} ---", log_file)?;
        }
    }

    // Create blast.log if it doesn't exist
    let blast_log = blast_dir.join("blast.log");
    if !blast_log.exists() {
        let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(&blast_log)?;

        writeln!(file, "--- Blast log initialized ---")?;
    }

    Ok(())
}

// Setup logging for the application
pub fn setup_for_mode(config: &Config, interactive: bool) -> Result<(), Box<dyn Error>> {
    // Ensure log directories and files exist
    ensure_log_files_exist(config)?;

    // Set environment variable for interactive mode
    if interactive {
        env::set_var("BLAST_INTERACTIVE", "1");
    }

    // Check if verbose mode is enabled via environment variable
    let verbose = env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1";
    
    // Set verbose mode if enabled via environment
    set_verbose_mode(verbose);

    // Determine log file path and mode
    let mode = if interactive {
        set_quiet_mode(true);
        RuntimeMode::Dashboard
    } else {
        RuntimeMode::Cli
    };

    // Get appropriate log path
    let log_path = if interactive {
        config.project_dir.join("storage").join("blast").join("blast.log")
    } else {
        config.project_dir.join("storage").join("logs").join("info.log")
    };

    // Initialize logging
    init(mode, Some(&log_path))?;

    Ok(())
}
