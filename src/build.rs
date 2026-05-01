use std::process::{Command, Stdio};

use crate::{
    configs::Config,
    error::{BlastError, BlastResult},
    logger,
};

pub fn run_build(config: &Config) -> BlastResult<()> {
    logger::info(&format!("Running cargo leptos build --release for {}...", config.project_name))?;

    let cargo_status = Command::new("cargo")
        .args(["leptos", "build", "--release", "--precompress"])
        .current_dir(&config.project_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !cargo_status.success() {
        return Err(BlastError::Subprocess {
            cmd: "cargo leptos build --release --precompress".to_string(),
            detail: "cargo leptos exited with non-zero status".to_string(),
        });
    }

    logger::success("Release build complete (binary + WASM bundle + precompressed assets)")?;
    Ok(())
}

pub fn run_package(config: &Config) -> BlastResult<()> {
    let binary_path = config.project_dir.join("target").join("release").join(&config.project_name);

    if !binary_path.exists() {
        return Err(BlastError::NotFound(format!("release binary not found at {}; run `blast build` first", binary_path.display())));
    }

    let site_dir = config.project_dir.join("target").join("site");
    if !site_dir.exists() {
        return Err(BlastError::NotFound(format!(
            "WASM bundle not found at {}; run `blast build` first (cargo leptos build --release)",
            site_dir.display()
        )));
    }

    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let archive_name = format!("release-{}-{}.tar.gz", config.project_name, timestamp);
    let archive_path = config.project_dir.join(&archive_name);

    let binary_rel = format!("target/release/{}", config.project_name);
    let mut tar_args: Vec<String> = vec![
        "-czf".to_string(),
        archive_path.to_string_lossy().to_string(),
        binary_rel,
        "target/site".to_string(),
    ];

    let env_example = config.project_dir.join(".env.example");
    if env_example.exists() {
        tar_args.push(".env.example".to_string());
    }

    let service_file = config.project_dir.join("deploy").join("systemd").join(format!("{}.service", config.project_name));
    if service_file.exists() {
        tar_args.push(format!("deploy/systemd/{}.service", config.project_name));
    }

    let status = Command::new("tar").args(&tar_args).current_dir(&config.project_dir).status()?;

    if !status.success() {
        return Err(BlastError::Subprocess {
            cmd: "tar".to_string(),
            detail: "tar exited with non-zero status".to_string(),
        });
    }

    logger::success(&format!("Package created: {}", archive_name))?;
    Ok(())
}
