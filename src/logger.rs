use crate::configs::Config;
use crate::progress;
use dialoguer::{FuzzySelect, Select};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

// Standard log files that are always present
pub const STANDARD_LOG_FILES: [&str; 5] = [
    "server.log",
    "error.log",
    "info.log",
    "debug.log",
    "warning.log",
];

// Get all the log files in the logs directory
pub fn get_log_files(config: &Config) -> Vec<PathBuf> {
    let logs_dir = config.project_dir.join("storage").join("logs");
    
    // Create logs directory if it doesn't exist
    if !logs_dir.exists() {
        if let Err(e) = fs::create_dir_all(&logs_dir) {
            eprintln!("Error creating logs directory: {}", e);
            return Vec::new();
        }
    }
    
    // Find all .log files in the logs directory
    let mut log_files = Vec::new();
    
    // Build glob pattern for .log files
    let mut builder = GlobSetBuilder::new();
    if let Ok(glob) = Glob::new("*.log") {
        builder.add(glob);
    }
    
    if let Ok(glob_set) = builder.build() {
        // Read the directory and filter for log files
        if let Ok(entries) = fs::read_dir(&logs_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name() {
                        let file_name_str = file_name.to_string_lossy();
                        if glob_set.is_match(file_name_str.as_ref()) {
                            log_files.push(path);
                        }
                    }
                }
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
    let progress = progress::ProgressManager::new_spinner();
    progress.set_message(&format!("Truncating log file: {}", log_path.display()));
    
    // Open the file in truncate mode
    let mut file = File::create(log_path)?;
    
    // Write a header to the file
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "--- Log file truncated at {} ---", timestamp)?;
    
    progress.success(&format!("Truncated log file: {}", log_path.display()));
    Ok(())
}

// Truncate all log files in the logs directory
pub fn truncate_all_logs(config: &Config) -> Result<(), Box<dyn Error>> {
    let log_files = get_log_files(config);
    
    if log_files.is_empty() {
        println!("No log files found");
        return Ok(());
    }
    
    for log_path in log_files {
        if let Err(e) = truncate_log_file(&log_path) {
            eprintln!("Error truncating {}: {}", log_path.display(), e);
        }
    }
    
    Ok(())
}

// Truncate a specific log file selected interactively
pub fn truncate_specific_log(config: &Config, file_name: Option<String>) -> Result<(), Box<dyn Error>> {
    let log_files = get_log_files(config);
    
    if log_files.is_empty() {
        println!("No log files found");
        return Ok(());
    }
    
    // If a specific file was provided, find and truncate it
    if let Some(file_name) = file_name {
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
            // See if the user forgot to add the .log extension
            let with_extension = if file_name.ends_with(".log") {
                file_name.clone()
            } else {
                format!("{}.log", file_name)
            };
            
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
    
    // Otherwise show an interactive menu
    let log_file_names: Vec<String> = log_files
        .iter()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .collect();
    
    // Add an "All Logs" option at the beginning
    let mut options = vec!["All Logs".to_string()];
    options.extend(log_file_names.clone());
    
    println!("Select a log file to truncate:");
    let selection = Select::new()
        .items(&options)
        .default(0)
        .interact()?;
    
    if selection == 0 {
        // User selected "All Logs"
        truncate_all_logs(config)
    } else {
        // User selected a specific log
        let selected_file = &log_files[selection - 1];
        truncate_log_file(selected_file)
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
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&log_path)?;
            
            writeln!(file, "--- Log file initialized: {} ---", log_file)?;
        }
    }
    
    // Create blast.log if it doesn't exist
    let blast_log = blast_dir.join("blast.log");
    if !blast_log.exists() {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&blast_log)?;
        
        writeln!(file, "--- Blast log initialized ---")?;
    }
    
    Ok(())
}