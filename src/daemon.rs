use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::process::CommandExt,
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
    // pkill --pgroup <pgid> --signal 0 = "is anyone in this process group still alive?"
    // (procps `/usr/bin/kill` does NOT accept negative PIDs, so we can't use `kill -0 -PID`)
    match Command::new("pkill").args(["--pgroup", &pid.to_string(), "--signal", "0"]).stdout(Stdio::null()).stderr(Stdio::null()).status() {
        Ok(status) => status.success(),
        Err(_io) => false, // allow: spawn failure for `pkill` leaves us no info; treat target as dead
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
    Command::new("pkill").args(["--pgroup", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status()?; // allow: target may already be dead, exit code carries no actionable info
    Ok(())
}

fn kill_force(pid: u32) -> BlastResult<()> {
    Command::new("pkill")
        .args(["--pgroup", &pid.to_string(), "--signal", "KILL"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?; // allow: target may already be dead, exit code carries no actionable info
    Ok(())
}

fn stop_pid_file(pid_file: &Path) -> BlastResult<bool> {
    let pid = match read_pid(pid_file)? {
        Some(p) => p,
        None => return Ok(false),
    };

    if !pid_alive(pid) {
        remove_pid_file(pid_file)?;
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

    remove_pid_file(pid_file)?;
    Ok(true)
}

pub fn stop_server(config: &Config) -> BlastResult<bool> {
    stop_pid_file(&pid_path(config))
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
            let mut c = Command::new("cargo");
            c.args(["run", "--release", "--no-default-features", "--features", "prod", "--bin", &config.project_name]);
            c
        }
        ServerMode::Watch => {
            let mut c = Command::new("cargo");
            let run_arg = format!("run --bin {}", &config.project_name);
            // whitelist watch dirs so vite's writes (frontend/.vite, node_modules/...) and
            // the running binary's writes (storage/...) don't re-trigger rebuilds in a loop
            c.arg("watch").arg("--watch").arg("src").arg("--watch").arg("Cargo.toml").arg("-x").arg(run_arg);
            c
        }
    };
    cmd.current_dir(&config.project_dir);
    cmd.env("CARGO_TERM_COLOR", "always");
    cmd
}

fn spawn_detached(mut cmd: Command, stdout: File, stderr: File, pid_file: &Path) -> BlastResult<u32> {
    cmd.stdin(Stdio::null()).stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr));
    cmd.process_group(0); // becomes its own process group leader so we can kill the whole subtree on stop

    let cmd_name = cmd.get_program().to_string_lossy().to_string();
    let child = cmd.spawn().map_err(|e| BlastError::Subprocess { cmd: cmd_name, detail: e.to_string() })?;
    let pid = child.id();
    std::mem::drop(child);

    fs::write(pid_file, pid.to_string())?;
    Ok(pid)
}
