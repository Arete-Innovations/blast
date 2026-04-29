use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    configs::Config,
    error::{BlastError, BlastResult},
};

#[derive(Clone, Copy, Debug)]
pub enum ServerMode {
    Dev,
    Prod,
    Watch,
}

impl ServerMode {
    fn label(self) -> &'static str {
        match self {
            ServerMode::Dev => "dev",
            ServerMode::Prod => "prod",
            ServerMode::Watch => "watch",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ServerStatus {
    Running(u32),
    Stopped,
}

const SERVER_NAME: &str = "server";

fn pid_path(config: &Config) -> PathBuf {
    config.project_dir.join("storage").join("blast").join(format!("{}.pid", SERVER_NAME))
}

fn log_path(config: &Config) -> PathBuf {
    config.project_dir.join("storage").join("logs").join(format!("{}.log", SERVER_NAME))
}

fn ensure_dirs(config: &Config) -> BlastResult<()> {
    fs::create_dir_all(config.project_dir.join("storage").join("blast"))?;
    fs::create_dir_all(config.project_dir.join("storage").join("logs"))?;
    Ok(())
}

fn read_pid(pid_file: &Path) -> BlastResult<Option<u32>> {
    let raw = match fs::read_to_string(pid_file) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    match raw.trim().parse::<u32>() {
        Ok(pid) => Ok(Some(pid)),
        Err(parse_err) => Err(BlastError::Invalid(format!("malformed pid in {}: {}", pid_file.display(), parse_err))),
    }
}

fn pid_alive(pid: u32) -> bool {
    match Command::new("kill").args(["-0", &pid.to_string()]).status() {
        Ok(status) => status.success(),
        Err(_io) => false, // allow: spawn failure for `kill -0` leaves us no info; treat target as dead
    }
}

fn remove_pid_file(pid_file: &Path) -> BlastResult<()> {
    match fs::remove_file(pid_file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn term(pid: u32) -> BlastResult<()> {
    Command::new("kill").arg(pid.to_string()).status()?; // allow: target may already be dead, exit code carries no actionable info
    Ok(())
}

fn kill_force(pid: u32) -> BlastResult<()> {
    Command::new("kill").args(["-9", &pid.to_string()]).status()?; // allow: target may already be dead, exit code carries no actionable info
    Ok(())
}

pub fn server_status(config: &Config) -> BlastResult<ServerStatus> {
    match read_pid(&pid_path(config))? {
        Some(pid) => {
            if pid_alive(pid) {
                Ok(ServerStatus::Running(pid))
            } else {
                Ok(ServerStatus::Stopped)
            }
        }
        None => Ok(ServerStatus::Stopped),
    }
}

pub fn stop_server(config: &Config) -> BlastResult<bool> {
    let pid_file = pid_path(config);
    let pid = match read_pid(&pid_file)? {
        Some(p) => p,
        None => return Ok(false),
    };

    if !pid_alive(pid) {
        remove_pid_file(&pid_file)?;
        return Ok(false);
    }

    term(pid)?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if !pid_alive(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    if pid_alive(pid) {
        kill_force(pid)?;
    }

    remove_pid_file(&pid_file)?;
    Ok(true)
}

pub fn start_server(config: &Config, mode: ServerMode) -> BlastResult<u32> {
    ensure_dirs(config)?;
    stop_server(config)?;

    let log_file_path = log_path(config);

    let mut log = OpenOptions::new().create(true).append(true).open(&log_file_path)?;
    let ts = chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]");
    writeln!(log, "\n{} ── server boot ({}) ──", ts, mode.label())?;
    drop(log);

    let stdout = File::options().create(true).append(true).open(&log_file_path)?;
    let stderr = stdout.try_clone()?;

    let cmd = build_command(config, mode);
    spawn_detached(cmd, stdout, stderr, &pid_path(config))
}

fn build_command(config: &Config, mode: ServerMode) -> Command {
    let mut cmd = match mode {
        ServerMode::Dev => {
            let mut c = Command::new("cargo");
            c.args(["run", "--bin", &config.project_name]);
            c
        }
        ServerMode::Prod => {
            let release_bin = config.project_dir.join("target").join("release").join(&config.project_name);
            if release_bin.exists() {
                Command::new(release_bin)
            } else {
                let mut c = Command::new("cargo");
                c.args(["run", "--release", "--bin", &config.project_name]);
                c
            }
        }
        ServerMode::Watch => {
            let mut c = Command::new("cargo");
            let run_arg = format!("run --bin {}", &config.project_name);
            c.arg("watch").arg("-x").arg(run_arg);
            c
        }
    };
    cmd.current_dir(&config.project_dir);
    cmd.env("CARGO_TERM_COLOR", "always");
    cmd
}

fn spawn_detached(mut cmd: Command, stdout: File, stderr: File, pid_file: &Path) -> BlastResult<u32> {
    cmd.stdin(Stdio::null()).stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr));

    let cmd_name = cmd.get_program().to_string_lossy().to_string();
    let child = cmd.spawn().map_err(|e| BlastError::Subprocess {
        cmd: cmd_name,
        detail: e.to_string(),
    })?;
    let pid = child.id();
    std::mem::drop(child);

    fs::write(pid_file, pid.to_string())?;
    Ok(pid)
}
