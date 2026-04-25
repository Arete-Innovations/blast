use crate::configs::Config;
use crate::error::{BlastError, BlastResult};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref SERVER_PROCESSES: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
}

pub fn check_zellij_installed() -> BlastResult<bool> {
    let output = Command::new("which").arg("zellij").output()?;
    Ok(output.status.success())
}

fn setup_logs(project_dir: &Path) -> BlastResult<()> {
    let logs_dir = project_dir.join("storage").join("logs");
    fs::create_dir_all(&logs_dir)?;

    let blast_dir = project_dir.join("storage").join("blast");
    fs::create_dir_all(&blast_dir)?;

    for filename in ["info.log", "server.log", "error.log", "debug.log", "warning.log"] {
        let log_file = logs_dir.join(filename);
        if !log_file.exists() {
            let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(&log_file)?;
            writeln!(file, "--- Log initialized: {} ---", filename)?;
        }
    }

    Ok(())
}

fn prepare_layout(project_dir: &Path) -> BlastResult<String> {
    let blast_dir = project_dir.join("storage").join("blast");
    let layout_path = blast_dir.join("dashboard.kdl");

    if !layout_path.exists() {
        return Err(BlastError::Dashboard(format!(
            "Zellij layout file not found at: {}\n\nThe dashboard.kdl file should be present in your project's storage/blast directory. This file should have been included in your project template. Please check your installation or create this file manually.",
            layout_path.display()
        )));
    }

    Ok(layout_path.to_string_lossy().to_string())
}

pub fn launch_dashboard(config: &Config) -> BlastResult<()> {
    if !check_zellij_installed()? {
        return Err(BlastError::Dashboard(
            "Zellij terminal multiplexer is not installed. Install it with 'cargo install zellij'".into(),
        ));
    }

    let project_dir = &config.project_dir;

    setup_logs(project_dir)?;

    let blast_log_path = project_dir.join("storage/blast/blast.log");

    let mut log_file = OpenOptions::new().create(true).write(true).append(true).open(&blast_log_path)?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(log_file, "\n\n--- Blast Dashboard Started: {} ---", now)?;
    writeln!(log_file, "Project: {}", config.project_name)?;
    writeln!(log_file, "Environment: {}", config.environment)?;
    writeln!(log_file, "-------------------------------------------")?;

    crate::logger::init(crate::logger::RuntimeMode::Dashboard, Some(&blast_log_path))?;

    std::env::set_var("BLAST_INTERACTIVE", "1");
    crate::logger::set_quiet_mode(true);

    let layout_path = prepare_layout(project_dir)?;

    println!("Launching Blast interactive dashboard...");

    println!("Starting fresh Blast dashboard session...");

    drop(Command::new("zellij").args(["kill-all-sessions", "-y"]).output());

    drop(Command::new("zellij").args(["delete-all-sessions", "-y"]).output());

    std::thread::sleep(std::time::Duration::from_millis(300));

    println!("Creating new Blast dashboard session...");

    use std::os::unix::process::CommandExt;

    println!("Launching Zellij with layout: {}", layout_path);

    let err = Command::new("zellij").arg("-l").arg(&layout_path).exec();

    return Err(BlastError::Dashboard(format!("Failed to exec Zellij: {}", err)));
}

pub fn start_server(config: &Config, is_dev: bool) -> BlastResult<u32> {
    let mut config_clone = config.clone();
    if let Err(e) = config_clone.reload_if_modified() {
        println!("Warning: Failed to reload configuration: {}", e);
    }
    let config = &config_clone;

    stop_server()?;

    let logs_dir = config.project_dir.join("storage").join("logs");
    fs::create_dir_all(&logs_dir)?;

    let blast_dir = config.project_dir.join("storage").join("blast");
    fs::create_dir_all(&blast_dir)?;

    let server_log_path = logs_dir.join("server.log");
    let error_log_path = logs_dir.join("error.log");

    drop(OpenOptions::new().create(true).append(true).open(&server_log_path)?);
    drop(OpenOptions::new().create(true).append(true).open(&error_log_path)?);

    let mut cmd = Command::new("bash");

    let show_warnings = match std::env::var("BLAST_SHOW_WARNINGS") {
        Ok(val) => val == "true",
        Err(_e) => config.show_compiler_warnings,
    };

    let (cargo_env, cargo_flags) = if show_warnings {
        ("".to_string(), "".to_string())
    } else {
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

    let output = cmd.output()?;
    let pid_str = String::from_utf8_lossy(&output.stdout);
    let pid = pid_str
        .trim()
        .parse::<u32>()
        .map_err(|e| BlastError::Dashboard(format!("failed to parse server PID: {}", e)))?;

    let mut processes = match SERVER_PROCESSES.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(BlastError::Dashboard(format!("SERVER_PROCESSES mutex poisoned: {}", e))),
    };
    processes.insert(config.project_name.clone(), pid);

    let pid_file_path = blast_dir.join("server.pid");
    fs::write(&pid_file_path, pid.to_string())?;

    let timestamp = chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]");
    let mut server_log = OpenOptions::new().create(true).append(true).open(&server_log_path)?;

    let env_name = if is_dev { "development" } else { "production" };
    writeln!(server_log, "{} Using {} configuration", timestamp, env_name)?;
    writeln!(server_log, "{} Server started with PID: {}", timestamp, pid)?;

    Ok(pid)
}

pub fn stop_server() -> BlastResult<()> {
    let mut processes = match SERVER_PROCESSES.lock() {
        Ok(guard) => guard,
        Err(e) => return Err(BlastError::Dashboard(format!("SERVER_PROCESSES mutex poisoned: {}", e))),
    };
    let mut stopped = false;

    for (name, pid) in processes.iter() {
        drop(Command::new("kill").arg(pid.to_string()).status());

        std::thread::sleep(std::time::Duration::from_millis(100));

        let ps_output = Command::new("ps").arg("-p").arg(pid.to_string()).output()?;

        if ps_output.status.success() {
            drop(Command::new("kill").arg("-9").arg(pid.to_string()).status());
        }

        let pid_file_path = "storage/blast/server.pid";
        if Path::new(&pid_file_path).exists() {
            drop(fs::remove_file(pid_file_path));
        }

        println!("Stopped server process '{}' with PID {}", name, pid);
        stopped = true;
    }

    if !stopped {
        let pid_file_path = "storage/blast/server.pid";
        if Path::new(pid_file_path).exists() {
            match fs::read_to_string(pid_file_path) {
                Ok(pid_str) => match pid_str.trim().parse::<u32>() {
                    Ok(pid) => {
                        drop(Command::new("kill").arg(pid.to_string()).status());
                        println!("Stopped orphaned server process with PID {}", pid);
                    }
                    Err(e) => {
                        println!("Warning: could not parse PID from server.pid: {}", e);
                    }
                },
                Err(e) => {
                    println!("Warning: could not read server.pid: {}", e);
                }
            }

            drop(fs::remove_file(pid_file_path));
        }
    }

    processes.clear();
    Ok(())
}
