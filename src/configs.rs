use crate::error::BlastResult;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use toml::Value;

#[derive(Clone, Debug)]
pub struct Config {
    pub environment: String,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub show_compiler_warnings: bool,
    pub last_modified: SystemTime,
}

impl Config {
    pub fn reload_if_modified(&mut self) -> BlastResult<bool> {
        let cargo_toml_path = self.project_dir.join("Cargo.toml");
        let metadata = fs::metadata(&cargo_toml_path)?;
        let modified = metadata.modified()?;

        if modified > self.last_modified {
            let new_config = build_config(&self.project_dir)?;
            self.environment = new_config.environment;
            self.project_name = new_config.project_name;
            self.show_compiler_warnings = new_config.show_compiler_warnings;
            self.last_modified = new_config.last_modified;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn toggle_environment(&mut self) -> BlastResult<()> {
        let old_env = self.environment.clone();
        self.environment = if self.environment == "dev" {
            "prod".to_string()
        } else {
            "dev".to_string()
        };
        crate::logger::success(&format!(
            "Environment toggled from {} to {}",
            old_env, self.environment
        ))?;
        Ok(())
    }
}

pub fn build_config(project_dir: &Path) -> BlastResult<Config> {
    let cargo_toml_path = project_dir.join("Cargo.toml");
    let cargo_str = fs::read_to_string(&cargo_toml_path)?;
    let cargo: Value = toml::from_str(&cargo_str)?;

    let project_name = match cargo
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
    {
        Some(name) => name.to_string(),
        None => "Unknown".to_string(),
    };

    let environment = match std::env::var("BLAST_ENV") {
        Ok(v) => v,
        Err(std::env::VarError::NotPresent) => "dev".to_string(),
        Err(e) => return Err(e.into()),
    };

    let metadata = fs::metadata(&cargo_toml_path)?;
    let last_modified = metadata.modified()?;

    Ok(Config {
        environment,
        project_name,
        project_dir: project_dir.to_path_buf(),
        show_compiler_warnings: true,
        last_modified,
    })
}

pub fn get_project_info() -> BlastResult<Config> {
    let cwd = std::env::current_dir()?;
    build_config(&cwd)
}
