use crate::configs::Config;
use chrono::Local;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use std::env;

// Type alias for consistent error handling
type BlastResult = Result<(), String>;
use std::fs::{self, OpenOptions};
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

// Global state
lazy_static! {
    static ref RUNTIME_MODE: Arc<Mutex<RuntimeMode>> = Arc::new(Mutex::new(RuntimeMode::Cli));
    static ref LOG_FILE_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    static ref QUIET_MODE: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref VERBOSE_MODE: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
}

// Standard log files
pub const STANDARD_LOG_FILES: [&str; 5] = ["server.log", "error.log", "info.log", "debug.log", "warning.log"];

// Initialize the logging system
pub fn init(mode: RuntimeMode, log_path: Option<&Path>) -> BlastResult {
    // Set runtime mode
    let mut current_mode = RUNTIME_MODE.lock().unwrap();
    *current_mode = mode;

    // If log path provided, initialize log file
    if let Some(path) = log_path {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Open log file
        let mut file = OpenOptions::new().create(true).write(true).append(true).open(path).map_err(|e| e.to_string())?;

        // Write session header
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        writeln!(file, "\n--- New Blast Session: {} ---", timestamp).map_err(|e| e.to_string())?;

        // Update global log path
        let mut log_path_guard = LOG_FILE_PATH.lock().unwrap();
        *log_path_guard = Some(path.to_path_buf());
    }

    Ok(())
}

// Environment checks
pub fn set_quiet_mode(quiet: bool) {
    let mut quiet_mode = QUIET_MODE.lock().unwrap();
    *quiet_mode = quiet;
}

pub fn set_verbose_mode(verbose: bool) {
    let mut verbose_mode = VERBOSE_MODE.lock().unwrap();
    *verbose_mode = verbose;
}

fn is_quiet() -> bool {
    let quiet_mode = QUIET_MODE.lock().unwrap();
    *quiet_mode
}

fn is_verbose() -> bool {
    let verbose_mode = VERBOSE_MODE.lock().unwrap();
    *verbose_mode || env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1"
}

fn get_mode() -> RuntimeMode {
    let mode = RUNTIME_MODE.lock().unwrap();
    *mode
}

// Get icon for log level
fn get_icon(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "🔍",
        LogLevel::Info => "ℹ️",
        LogLevel::Warning => "⚠️",
        LogLevel::Error => "❌",
        LogLevel::Success => "✅",
    }
}

// Simple logging function
pub fn log(level: LogLevel, message: &str) -> BlastResult {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let icon = get_icon(level);
    
    // Format log message
    let log_msg = format!("[{}] [{}] {}", timestamp, level_to_string(level), message);
    
    // Write to log file if in dashboard mode
    if get_mode() == RuntimeMode::Dashboard {
        if let Some(log_path) = &*LOG_FILE_PATH.lock().unwrap() {
            if let Ok(mut file) = OpenOptions::new().create(true).write(true).append(true).open(log_path) {
                writeln!(file, "{}", log_msg).map_err(|e| e.to_string())?;
            }
        }
        return Ok(());
    }
    
    // CLI mode output handling
    if is_quiet() {
        return Ok(());
    }
    
    // Only show debug in verbose mode
    if level == LogLevel::Debug && !is_verbose() {
        return Ok(());
    }
    
    // Only show info in verbose mode unless critical
    if level == LogLevel::Info && !is_verbose() && !message.contains("critical") {
        return Ok(());
    }
    
    // Print to console with appropriate styling
    match level {
        LogLevel::Debug => println!("{} {}", icon, message),
        LogLevel::Info => println!("{} {}", icon, message),
        LogLevel::Warning => println!("{} {}", icon, style(message).yellow()),
        LogLevel::Error => println!("{} {}", icon, style(message).red().bold()),
        LogLevel::Success => println!("{} {}", icon, style(message).green()),
    }
    
    Ok(())
}

fn level_to_string(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "DEBUG",
        LogLevel::Info => "INFO",
        LogLevel::Warning => "WARNING",
        LogLevel::Error => "ERROR",
        LogLevel::Success => "SUCCESS",
    }
}

// Helper functions for specific log levels
pub fn debug(message: &str) -> BlastResult {
    log(LogLevel::Debug, message)
}

pub fn info(message: &str) -> BlastResult {
    log(LogLevel::Info, message)
}

pub fn warning(message: &str) -> BlastResult {
    log(LogLevel::Warning, message)
}

pub fn error(message: &str) -> BlastResult {
    log(LogLevel::Error, message)
}

pub fn success(message: &str) -> BlastResult {
    log(LogLevel::Success, message)
}

// Simple progress bar implementation
pub fn create_progress(steps: Option<u64>) -> Progress {
    Progress::new(steps)
}

#[derive(Clone)]
pub struct Progress {
    bar: ProgressBar,
}

impl Progress {
    fn new(steps: Option<u64>) -> Self {
        let bar = match steps {
            Some(total) => {
                let pb = ProgressBar::new(total);
                let style = ProgressStyle::default_bar()
                    .template("{spinner:.green} {wide_msg} [{pos}/{len}]")
                    .unwrap()
                    .progress_chars("=>-");
                pb.set_style(style);
                pb
            }
            None => {
                let pb = ProgressBar::new_spinner();
                let style = ProgressStyle::default_spinner()
                    .template("{spinner:.green} {wide_msg}")
                    .unwrap();
                pb.set_style(style);
                pb.enable_steady_tick(std::time::Duration::from_millis(100));
                pb
            }
        };

        Progress {
            bar,
        }
    }

    pub fn set_message(&mut self, msg: &str) -> &mut Self {
        // Dashboard mode - log to file
        if get_mode() == RuntimeMode::Dashboard {
            let _ = info(msg);
            return self;
        }
        
        // CLI mode - update progress bar
        if !is_quiet() {
            self.bar.set_message(msg.to_string());
        }
        
        self
    }

    pub fn inc(&mut self, delta: u64) -> &mut Self {
        // Dashboard mode - just log
        if get_mode() == RuntimeMode::Dashboard {
            return self;
        }
        
        // CLI mode - update progress bar
        if !is_quiet() {
            self.bar.inc(delta);
        }
        
        self
    }

    pub fn success(&mut self, msg: &str) {
        // Dashboard mode - log to file
        if get_mode() == RuntimeMode::Dashboard {
            let _ = success(msg);
            return;
        }
        
        // CLI mode - finish and show message
        if !is_quiet() {
            self.bar.finish_and_clear();
            println!("{} {}", get_icon(LogLevel::Success), msg);
        }
    }

    pub fn error(&mut self, msg: &str) {
        // Dashboard mode - log to file
        if get_mode() == RuntimeMode::Dashboard {
            let _ = error(msg);
            return;
        }
        
        // CLI mode - finish and show error
        if !is_quiet() {
            self.bar.finish_and_clear();
            eprintln!("{} {}", get_icon(LogLevel::Error), style(msg).red().bold());
        }
    }

    pub fn warning(&mut self, msg: &str) -> BlastResult {
        // Dashboard mode - log to file
        if get_mode() == RuntimeMode::Dashboard {
            warning(msg)?;
            return Ok(());
        }
        
        // CLI mode - suspend and show warning
        if !is_quiet() {
            self.bar.suspend(|| {
                println!("{} {}", get_icon(LogLevel::Warning), style(msg).yellow());
            });
        }
        
        Ok(())
    }
}

// File system operations for logs
pub fn ensure_log_files_exist(config: &Config) -> BlastResult {
    let logs_dir = config.project_dir.join("storage").join("logs");
    let blast_dir = config.project_dir.join("storage").join("blast");
    
    // Create directories
    fs::create_dir_all(&logs_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&blast_dir).map_err(|e| e.to_string())?;
    
    // Create standard log files
    for log_file in STANDARD_LOG_FILES.iter() {
        let log_path = logs_dir.join(log_file);
        if !log_path.exists() {
            let mut file = OpenOptions::new().create(true).write(true).open(&log_path).map_err(|e| e.to_string())?;
            writeln!(file, "--- Log file initialized: {} ---", log_file).map_err(|e| e.to_string())?;
        }
    }
    
    // Create blast log
    let blast_log = blast_dir.join("blast.log");
    if !blast_log.exists() {
        let mut file = OpenOptions::new().create(true).write(true).open(&blast_log).map_err(|e| e.to_string())?;
        writeln!(file, "--- Blast log initialized ---").map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

pub fn setup_for_mode(config: &Config, interactive: bool) -> BlastResult {
    // Ensure log files exist
    ensure_log_files_exist(config)?;
    
    // Set environment variable
    if interactive {
        env::set_var("BLAST_INTERACTIVE", "1");
    }
    
    // Check verbose mode
    let verbose = env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1";
    set_verbose_mode(verbose);
    
    // Set mode and log path
    let mode = if interactive {
        set_quiet_mode(true);
        RuntimeMode::Dashboard
    } else {
        RuntimeMode::Cli
    };
    
    let log_path = if interactive {
        config.project_dir.join("storage").join("blast").join("blast.log")
    } else {
        config.project_dir.join("storage").join("logs").join("info.log")
    };
    
    // Initialize logger
    init(mode, Some(&log_path))?;
    
    Ok(())
}

// Log file management functions
pub fn get_log_files(config: &Config) -> Vec<PathBuf> {
    let logs_dir = config.project_dir.join("storage").join("logs");
    let blast_dir = config.project_dir.join("storage").join("blast");
    
    let mut log_files = Vec::new();
    
    // Read logs directory
    if logs_dir.exists() {
        if let Ok(entries) = fs::read_dir(&logs_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "log") {
                    log_files.push(path);
                }
            }
        }
    }
    
    // Add blast.log
    let blast_log = blast_dir.join("blast.log");
    if blast_log.exists() {
        log_files.push(blast_log);
    }
    
    log_files
}

pub fn truncate_log_file(log_path: &Path) -> BlastResult {
    info(&format!("Truncating log file: {}", log_path.display()))?;
    
    // Create empty file
    let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(log_path).map_err(|e| e.to_string())?;
    
    // Write header
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "--- Log file truncated at {} ---", timestamp).map_err(|e| e.to_string())?;
    
    success(&format!("Truncated log file: {}", log_path.display()))?;
    Ok(())
}

pub fn truncate_all_logs(config: &Config) -> BlastResult {
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

pub fn truncate_specific_log(config: &Config, file_name: Option<String>) -> BlastResult {
    // Truncate all if no specific file
    if file_name.is_none() {
        return truncate_all_logs(config);
    }
    
    let file_name = file_name.unwrap();
    let logs_dir = config.project_dir.join("storage").join("logs");
    let blast_dir = config.project_dir.join("storage").join("blast");
    
    // Try with and without .log extension
    let with_ext = if file_name.ends_with(".log") { 
        file_name.clone() 
    } else { 
        format!("{}.log", file_name) 
    };
    
    // Check different possible locations
    let paths = [
        logs_dir.join(&file_name),
        blast_dir.join(&file_name),
        logs_dir.join(&with_ext),
        blast_dir.join(&with_ext),
    ];
    
    for path in paths.iter() {
        if path.exists() {
            return truncate_log_file(path);
        }
    }
    
    // Not found
    error(&format!("Log file not found: {}", file_name))?;
    Ok(())
}