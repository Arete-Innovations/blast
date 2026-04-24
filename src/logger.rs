use crate::configs::Config;
use crate::error::{BlastError, BlastResult};
use chrono::Local;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use std::env;
use std::thread;
use std::time::Duration;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Cli,
    Dashboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Success,
}

lazy_static! {
    static ref RUNTIME_MODE: Arc<Mutex<RuntimeMode>> = Arc::new(Mutex::new(RuntimeMode::Cli));
    static ref LOG_FILE_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    static ref QUIET_MODE: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref VERBOSE_MODE: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
}

pub const STANDARD_LOG_FILES: [&str; 5] = ["server.log", "error.log", "info.log", "debug.log", "warning.log"];

pub fn init(mode: RuntimeMode, log_path: Option<&Path>) -> BlastResult<()> {
    let Ok(mut current_mode) = RUNTIME_MODE.lock() else {
        return Err(BlastError::Project("RUNTIME_MODE mutex poisoned".to_string()));
    };
    *current_mode = mode;

    let Some(path) = log_path else {
        return Ok(());
    };

    match path.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {}
    }

    let mut file = OpenOptions::new().create(true).write(true).append(true).open(path)?;

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "\n--- New Blast Session: {} ---", timestamp)?;

    let Ok(mut log_path_guard) = LOG_FILE_PATH.lock() else {
        return Err(BlastError::Project("LOG_FILE_PATH mutex poisoned".to_string()));
    };
    *log_path_guard = Some(path.to_path_buf());

    Ok(())
}

pub fn set_quiet_mode(quiet: bool) {
    let Ok(mut quiet_mode) = QUIET_MODE.lock() else {
        return;
    };
    *quiet_mode = quiet;
}

pub fn set_verbose_mode(verbose: bool) {
    let Ok(mut verbose_mode) = VERBOSE_MODE.lock() else {
        return;
    };
    *verbose_mode = verbose;
}

fn is_quiet() -> bool {
    let Ok(quiet_mode) = QUIET_MODE.lock() else {
        return false;
    };
    *quiet_mode
}

pub fn is_verbose() -> bool {
    let Ok(verbose_mode) = VERBOSE_MODE.lock() else {
        return false;
    };
    let from_flag = *verbose_mode;
    let from_env = env::var("BLAST_VERBOSE").is_ok_and(|v| v == "1");
    from_flag || from_env
}

fn get_mode() -> RuntimeMode {
    let Ok(mode) = RUNTIME_MODE.lock() else {
        return RuntimeMode::Cli;
    };
    *mode
}

fn get_icon(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "🔍",
        LogLevel::Info => "ℹ️",
        LogLevel::Warning => "⚠️",
        LogLevel::Error => "❌",
        LogLevel::Success => "✅",
    }
}

pub fn log(level: LogLevel, message: &str) -> BlastResult<()> {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let icon = get_icon(level);

    let log_msg = format!("[{}] [{}] {}", timestamp, level_to_string(level), message);

    if get_mode() == RuntimeMode::Dashboard {
        let Ok(guard) = LOG_FILE_PATH.lock() else {
            return Ok(());
        };
        let Some(log_path) = &*guard else {
            return Ok(());
        };
        match OpenOptions::new().create(true).write(true).append(true).open(log_path) {
            Ok(mut file) => {
                writeln!(file, "{}", log_msg)?;
            }
            Err(e) => return Err(BlastError::Io(e)),
        }
        return Ok(());
    }

    if is_quiet() {
        return Ok(());
    }

    if level == LogLevel::Debug && !is_verbose() {
        return Ok(());
    }

    if level == LogLevel::Info && !is_verbose() && !message.contains("critical") {
        return Ok(());
    }

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

pub fn debug(message: &str) -> BlastResult<()> {
    log(LogLevel::Debug, message)
}

pub fn info(message: &str) -> BlastResult<()> {
    log(LogLevel::Info, message)
}

pub fn warning(message: &str) -> BlastResult<()> {
    log(LogLevel::Warning, message)
}

pub fn error(message: &str) -> BlastResult<()> {
    log(LogLevel::Error, message)
}

pub fn success(message: &str) -> BlastResult<()> {
    log(LogLevel::Success, message)
}

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
                let style = match ProgressStyle::default_bar()
                    .template("{spinner:.green} {wide_msg} [{pos}/{len}]")
                {
                    Ok(s) => s.progress_chars("=>-"),
                    Err(_e) => ProgressStyle::default_bar(),
                };
                pb.set_style(style);
                pb
            }
            None => {
                let pb = ProgressBar::new_spinner();
                let style = match ProgressStyle::default_spinner()
                    .template("{spinner:.green} {wide_msg}")
                {
                    Ok(s) => s,
                    Err(_e) => ProgressStyle::default_spinner(),
                };
                pb.set_style(style);
                pb.enable_steady_tick(std::time::Duration::from_millis(100));
                pb
            }
        };

        Progress { bar }
    }

    pub fn set_message(&mut self, msg: &str) -> &mut Self {
        if get_mode() == RuntimeMode::Dashboard {
            drop(info(msg));
            return self;
        }

        if !is_quiet() {
            self.bar.set_message(msg.to_string());
        }

        self
    }

    pub fn inc(&mut self, delta: u64) -> &mut Self {
        if get_mode() == RuntimeMode::Dashboard {
            return self;
        }

        if !is_quiet() {
            self.bar.inc(delta);
        }

        self
    }

    pub fn success(&mut self, msg: &str) {
        if get_mode() == RuntimeMode::Dashboard {
            drop(success(msg));
            return;
        }

        if !is_quiet() {
            self.bar.finish_and_clear();
            println!("{} {}", get_icon(LogLevel::Success), msg);
        }
    }

    pub fn error(&mut self, msg: &str) {
        if get_mode() == RuntimeMode::Dashboard {
            drop(error(msg));
            return;
        }

        if !is_quiet() {
            self.bar.finish_and_clear();
            eprintln!("{} {}", get_icon(LogLevel::Error), style(msg).red().bold());
        }
    }

    pub fn warning(&mut self, msg: &str) -> BlastResult<()> {
        if get_mode() == RuntimeMode::Dashboard {
            warning(msg)?;
            return Ok(());
        }

        if !is_quiet() {
            self.bar.suspend(|| {
                println!("{} {}", get_icon(LogLevel::Warning), style(msg).yellow());
            });
        }

        Ok(())
    }
}

pub fn ensure_log_files_exist(config: &Config) -> BlastResult<()> {
    let logs_dir = config.project_dir.join("storage").join("logs");
    let blast_dir = config.project_dir.join("storage").join("blast");

    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&blast_dir)?;

    for log_file in STANDARD_LOG_FILES.iter() {
        let log_path = logs_dir.join(log_file);
        if !log_path.exists() {
            let mut file = OpenOptions::new().create(true).write(true).open(&log_path)?;
            writeln!(file, "--- Log file initialized: {} ---", log_file)?;
        }
    }

    let blast_log = blast_dir.join("blast.log");
    if !blast_log.exists() {
        let mut file = OpenOptions::new().create(true).write(true).open(&blast_log)?;
        writeln!(file, "--- Blast log initialized ---")?;
    }

    Ok(())
}

pub fn setup_for_mode(config: &Config, interactive: bool) -> BlastResult<()> {
    ensure_log_files_exist(config)?;

    if interactive {
        env::set_var("BLAST_INTERACTIVE", "1");
    }

    let verbose = env::var("BLAST_VERBOSE").is_ok_and(|v| v == "1");
    set_verbose_mode(verbose);

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

    init(mode, Some(&log_path))?;

    Ok(())
}

pub fn get_log_files(config: &Config) -> Vec<PathBuf> {
    let logs_dir = config.project_dir.join("storage").join("logs");
    let blast_dir = config.project_dir.join("storage").join("blast");

    let mut log_files = Vec::new();

    if logs_dir.exists() {
        let Ok(entries) = fs::read_dir(&logs_dir) else {
            return log_files;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "log") {
                log_files.push(path);
            }
        }
    }

    let blast_log = blast_dir.join("blast.log");
    if blast_log.exists() {
        log_files.push(blast_log);
    }

    log_files
}

pub fn truncate_log_file(log_path: &Path) -> BlastResult<()> {
    info(&format!("Truncating log file: {}", log_path.display()))?;

    let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(log_path)?;

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "--- Log file truncated at {} ---", timestamp)?;

    success(&format!("Truncated log file: {}", log_path.display()))?;
    Ok(())
}

pub fn truncate_all_logs(config: &Config) -> BlastResult<()> {
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

pub fn truncate_specific_log(config: &Config, file_name: Option<String>) -> BlastResult<()> {
    let Some(file_name) = file_name else {
        return truncate_all_logs(config);
    };

    let logs_dir = config.project_dir.join("storage").join("logs");
    let blast_dir = config.project_dir.join("storage").join("blast");

    let with_ext = if file_name.ends_with(".log") {
        file_name.clone()
    } else {
        format!("{}.log", file_name)
    };

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

    Err(BlastError::NotFound(format!("log file not found: {}", file_name)))
}

pub fn view_logs_enhanced(level: &str, config: &Config) -> BlastResult<()> {
    let logs_dir = config.project_dir.join("storage").join("logs");
    let log_file = format!("{}.log", level.to_lowercase());
    let log_path = logs_dir.join(&log_file);

    if !log_path.exists() {
        return Err(BlastError::NotFound(format!("log file not found: {}", log_file)));
    }

    info(&format!("Following {} logs (Ctrl+C to stop)...", level))?;

    let mut last_size = match fs::metadata(&log_path) {
        Ok(metadata) => {
            let size = metadata.len();
            let content = match fs::read_to_string(&log_path) {
                Ok(c) => c,
                Err(e) => return Err(BlastError::Io(e)),
            };
            if !content.trim().is_empty() {
                println!("{}", style(format!("=== {} LOGS ===", level.to_uppercase())).bold().cyan());
                println!();
                for line in content.lines() {
                    if line.trim().is_empty() || line.starts_with("---") {
                        continue;
                    }
                    format_log_entry(line);
                }
            }
            size
        }
        Err(e) => return Err(BlastError::Io(e)),
    };

    println!("{}", style("--- Following new entries ---").dim());

    loop {
        match fs::metadata(&log_path) {
            Ok(metadata) => {
                let current_size = metadata.len();
                if current_size > last_size {
                    match fs::read_to_string(&log_path) {
                        Ok(content) => {
                            let new_content = &content[(last_size as usize)..];
                            for line in new_content.lines() {
                                if line.trim().is_empty() || line.starts_with("---") {
                                    continue;
                                }
                                format_log_entry(line);
                            }
                        }
                        Err(e) => return Err(BlastError::Io(e)),
                    }
                    last_size = current_size;
                }
            }
            Err(e) => return Err(BlastError::Io(e)),
        }

        thread::sleep(Duration::from_millis(500));
    }
}

fn format_log_entry(line: &str) {
    let Some(second_bracket_start) = line.find("] [") else {
        println!("{}", line);
        return;
    };
    let Some(second_bracket_end) = line[second_bracket_start + 3..].find(']') else {
        println!("{}", line);
        return;
    };

    let file_location = &line[second_bracket_start + 3..second_bracket_start + 3 + second_bracket_end];
    let rest = &line[second_bracket_start + 3 + second_bracket_end + 1..].trim();

    let parts: Vec<&str> = rest.split(" → ").collect();

    if parts.len() >= 3 {
        let message = parts[0];
        let context_timing = parts[1];
        let trace_items = &parts[2..];

        println!("📍[{}] {}", file_location, message);
        println!("┗┳╾ {}", style(context_timing).cyan());

        for (i, trace_item) in trace_items.iter().enumerate() {
            let indent = " ".repeat(i + 1);
            let connector = if i == trace_items.len() - 1 { "┗━╾" } else { "┗┳╾" };
            println!("{}{} {}", indent, style(connector).dim(), style(trace_item).yellow());
        }
    } else if parts.len() == 2 {
        let message = parts[0];
        let context_timing = parts[1];

        println!("📍[{}] {}", file_location, message);
        println!("┗━╾ {}", style(context_timing).cyan());
    } else {
        println!("📍[{}] {}", file_location, rest);
    }

    println!();
}
