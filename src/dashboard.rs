use crate::configs::Config;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

// Global state for server processes
lazy_static! {
    static ref SERVER_PROCESSES: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
}

// Standard log file paths structure
// Keeping this structure as it may be used in the future
#[allow(dead_code)]
pub struct LogPaths {
    pub info_log: PathBuf,
    pub server_log: PathBuf,
    pub error_log: PathBuf,
    pub debug_log: PathBuf,
    pub warning_log: PathBuf,
}

// Manage external dependencies
pub fn check_zellij_installed() -> bool {
    let output = Command::new("which").arg("zellij").output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

// Create or verify log files and directories
fn setup_logs(project_dir: &Path) -> Result<LogPaths, Box<dyn Error>> {
    // Create logs directory in storage
    let logs_dir = project_dir.join("storage").join("logs");
    fs::create_dir_all(&logs_dir)?;

    // Create dashboard state directory
    let blast_dir = project_dir.join("storage").join("blast");
    fs::create_dir_all(&blast_dir)?;

    // Define log file paths
    let info_log = logs_dir.join("info.log");
    let server_log = logs_dir.join("server.log");
    let error_log = logs_dir.join("error.log");
    let debug_log = logs_dir.join("debug.log");
    let warning_log = logs_dir.join("warning.log");

    // Create empty log files if they don't exist
    for log_file in [&info_log, &error_log, &debug_log, &warning_log, &server_log].iter() {
        if !log_file.exists() {
            let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(log_file)?;

            writeln!(file, "--- Log initialized: {} ---", log_file.file_name().unwrap_or_default().to_string_lossy())?;
        }
    }

    Ok(LogPaths {
        info_log,
        server_log,
        error_log,
        debug_log,
        warning_log,
    })
}

// Create Zellij layout file in the blast folder
fn prepare_layout(project_dir: &Path) -> Result<String, Box<dyn Error>> {
    // Store in the blast directory
    let blast_dir = project_dir.join("storage").join("blast");
    let layout_path = blast_dir.join("dashboard.kdl");

    // Get the base layout content (already set up with correct paths)
    let layout_content = include_str!("layouts/blast_dashboard.kdl").to_string();

    // Write the layout file
    fs::write(&layout_path, layout_content)?;

    Ok(layout_path.to_string_lossy().to_string())
}

// Main function to launch the interactive dashboard
pub fn launch_dashboard(config: &Config) -> Result<(), Box<dyn Error>> {
    use crate::output::{self, OutputMode};

    // Check if zellij is installed
    if !check_zellij_installed() {
        return Err("Zellij terminal multiplexer is not installed. Install it with 'cargo install zellij'".into());
    }

    // Get project directory
    let project_dir = &config.project_dir;

    // Set up log files
    let _log_paths = setup_logs(project_dir)?;

    // Create a single log file for dashboard output
    let blast_log_path = project_dir.join("storage/blast/blast.log");

    // Initialize the blast log file with a header
    let mut log_file = OpenOptions::new().create(true).write(true).append(true).open(&blast_log_path)?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(log_file, "\n\n--- Blast Dashboard Started: {} ---", now)?;
    writeln!(log_file, "Project: {}", config.project_name)?;
    writeln!(log_file, "Environment: {}", config.environment)?;
    writeln!(log_file, "-------------------------------------------")?;

    // Set output mode to log file for all operations
    output::set_output_mode(OutputMode::LogFile);
    output::set_log_file_path(&blast_log_path)?;

    // Important: Set interactive mode and quiet mode for dashboard
    std::env::set_var("BLAST_INTERACTIVE", "1");
    output::set_quiet_mode(true);

    // We don't need the symlink anymore since we're using a single log file

    // Prepare layout
    let layout_path = prepare_layout(project_dir)?;

    // Launch zellij with the layout and custom session name
    println!("Launching Blast interactive dashboard...");

    // No need for a session name when using exec()

    // Create a completely fresh Blast dashboard
    println!("Starting fresh Blast dashboard session...");

    // First try to kill any active session with our name
    let _ = Command::new("zellij").args(["kill-all-sessions", "-y"]).output();

    // Then delete any dead sessions with our name
    let _ = Command::new("zellij").args(["delete-all-sessions", "-y"]).output();

    // Sleep briefly to ensure the deletion completes
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Create a fresh new session with our layout
    println!("Creating new Blast dashboard session...");

    // Use std::process::Command::exec to replace the current process with Zellij
    // This way the process won't exit until Zellij exits
    use std::os::unix::process::CommandExt;

    println!("Launching Zellij with layout: {}", layout_path);

    // This will replace the current process with Zellij
    let err = Command::new("zellij").arg("-l").arg(&layout_path).exec();

    // If exec() returns, it means it failed
    return Err(format!("Failed to exec Zellij: {}", err).into());
}

// Start a server process and redirect output to standard log files
pub fn start_server(config: &Config, is_dev: bool) -> Result<u32, Box<dyn Error>> {
    // Ensure we're using the latest configuration
    let mut config_clone = config.clone();
    if let Err(e) = config_clone.reload_if_modified() {
        // Log the error but continue with the existing config
        println!("Warning: Failed to reload configuration: {}", e);
    }
    // Use the refreshed config
    let config = &config_clone;

    // Kill any existing server process
    stop_server()?;

    // Create logs directory if it doesn't exist
    let logs_dir = config.project_dir.join("storage").join("logs");
    fs::create_dir_all(&logs_dir)?;

    // Create storage directory for PIDs
    let blast_dir = config.project_dir.join("storage").join("blast");
    fs::create_dir_all(&blast_dir)?;

    // Get log paths
    let server_log_path = logs_dir.join("server.log");
    let error_log_path = logs_dir.join("error.log");

    // Open log files (make sure they exist)
    let _ = OpenOptions::new().create(true).append(true).open(&server_log_path)?;
    let _ = OpenOptions::new().create(true).append(true).open(&error_log_path)?;

    // Start the server process using script to preserve colors
    // but still properly detach it and prevent interactive menu from affecting it
    let mut cmd = Command::new("bash");

    // Determine cargo flags based on config setting
    // We still check environment variable as a way to override the config setting if needed
    let env_setting = std::env::var("BLAST_SHOW_WARNINGS").ok();
    let show_warnings = env_setting.map(|v| v == "true").unwrap_or(config.show_compiler_warnings);

    // Set up command with appropriate flags to control warnings
    let (cargo_env, cargo_flags) = if show_warnings {
        // Show warnings (default behavior)
        ("".to_string(), "".to_string())
    } else {
        // Hide warnings - both set RUSTFLAGS and use --quiet
        ("RUSTFLAGS=\"-Awarnings\"".to_string(), "--quiet".to_string())
    };

    let run_command = if is_dev {
        format!(
            "nohup script -q -f -c \"{} cargo run {} --bin {}\" storage/logs/server.log </dev/null >/dev/null 2>&1 & echo $!",
            cargo_env, cargo_flags, &config.project_name
        )
    } else {
        format!(
            "nohup script -q -f -c \"{} cargo run {} --release --bin {}\" storage/logs/server.log </dev/null >/dev/null 2>&1 & echo $!",
            cargo_env, cargo_flags, &config.project_name
        )
    };

    cmd.args(["-c", &run_command]);

    // Capture the PID from the output of the command
    let output = cmd.output()?;
    let pid_str = String::from_utf8_lossy(&output.stdout);
    let pid = pid_str.trim().parse::<u32>().map_err(|_| "Failed to parse PID")?;

    // Store the PID
    let mut processes = SERVER_PROCESSES.lock().unwrap();
    processes.insert(config.project_name.clone(), pid);

    // Also store the PID in a file for the interactive menu to access
    let pid_file_path = blast_dir.join("server.pid");
    fs::write(&pid_file_path, pid.to_string())?;

    // Log to the server log
    let timestamp = chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]");
    let mut server_log = OpenOptions::new().create(true).append(true).open(&server_log_path)?;

    writeln!(server_log, "{} Server started with PID: {}", timestamp, pid)?;

    Ok(pid)
}

// Stop a running server process
pub fn stop_server() -> Result<(), Box<dyn Error>> {
    let mut processes = SERVER_PROCESSES.lock().unwrap();
    let mut stopped = false;

    // Iterate through and stop each process
    for (name, pid) in processes.iter() {
        // Try to kill the process
        let _ = Command::new("kill").arg(pid.to_string()).status();

        // Give the process a short time to terminate gracefully
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Force kill if still running
        let ps_output = Command::new("ps").arg("-p").arg(pid.to_string()).output()?;

        if ps_output.status.success() {
            let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
        }

        // Remove PID file if it exists
        let pid_file_path = "storage/blast/server.pid";
        if Path::new(&pid_file_path).exists() {
            let _ = fs::remove_file(pid_file_path);
        }

        println!("Stopped server process '{}' with PID {}", name, pid);
        stopped = true;
    }

    // If no processes were stopped, check for orphaned PID file
    if !stopped {
        let pid_file_path = "storage/blast/server.pid";
        if Path::new(pid_file_path).exists() {
            // Read the PID from the file
            if let Ok(pid_str) = fs::read_to_string(pid_file_path) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    // Try to kill it
                    let _ = Command::new("kill").arg(pid.to_string()).status();

                    println!("Stopped orphaned server process with PID {}", pid);
                }
            }

            // Remove the PID file
            let _ = fs::remove_file(pid_file_path);
        }
    }

    processes.clear();
    Ok(())
}
