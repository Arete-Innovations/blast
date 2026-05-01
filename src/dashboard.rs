use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::Command,
};

use crate::{
    configs::Config,
    error::{BlastError, BlastResult},
};

pub fn check_zellij_installed() -> BlastResult<bool> {
    let output = Command::new("which").arg("zellij").output()?;
    Ok(output.status.success())
}

fn setup_logs(project_dir: &Path) -> BlastResult<()> {
    let logs_dir = project_dir.join("storage").join("logs");
    fs::create_dir_all(&logs_dir)?;

    let blast_dir = project_dir.join("storage").join("blast");
    fs::create_dir_all(&blast_dir)?;

    for filename in ["info.log", "server.log", "error.log", "debug.log", "warning.log", "fe.log", "routes.log"] {
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
            "Zellij layout file not found at: {}\n\nThe dashboard.kdl file should be present in your project's storage/blast directory. This file should have been included in your project template. Please check your \
             installation or create this file manually.",
            layout_path.display()
        )));
    }

    Ok(layout_path.to_string_lossy().to_string())
}

pub fn launch_dashboard(config: &Config) -> BlastResult<()> {
    if !check_zellij_installed()? {
        return Err(BlastError::Dashboard("Zellij terminal multiplexer is not installed. Install it with 'cargo install zellij'".into()));
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

    let auto_mode = if config.environment == "prod" { crate::daemon::ServerMode::Prod } else { crate::daemon::ServerMode::Watch };
    match crate::daemon::start_server(config, auto_mode) {
        Ok(pid) => println!("Auto-started backend daemon (PID {})", pid),
        Err(e) => eprintln!("warning: failed to auto-start backend daemon: {}", e),
    }

    println!("Launching Blast interactive dashboard...");

    println!("Starting fresh Blast dashboard session...");

    drop(Command::new("zellij").args(["kill-all-sessions", "-y"]).output());

    drop(Command::new("zellij").args(["delete-all-sessions", "-y"]).output());

    std::thread::sleep(std::time::Duration::from_millis(300));

    println!("Creating new Blast dashboard session...");
    println!("Launching Zellij with layout: {}", layout_path);

    let zellij_status = Command::new("zellij").arg("-l").arg(&layout_path).status()?;

    println!("\nDashboard exited. Stopping backend daemon...");
    match crate::daemon::stop_server(config) {
        Ok(true) => println!("Backend daemon stopped."),
        Ok(false) => println!("No backend daemon was running."),
        Err(e) => eprintln!("warning: failed to stop backend daemon: {}", e),
    }

    if zellij_status.success() {
        Ok(())
    } else {
        Err(BlastError::Dashboard(format!("zellij exited with status {:?}", zellij_status.code())))
    }
}
