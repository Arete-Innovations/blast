use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use toml::Value;

type ConfigResult<T> = Result<T, String>;

// TODO(blueprint): replace with Blueprint IR reading per SPEC_CONFIG.md
#[derive(Clone, Debug)]
pub struct Config {
    pub environment: String,
    pub project_name: String,
    pub project_dir: PathBuf,
    pub show_compiler_warnings: bool,
    pub last_modified: SystemTime,
}

impl Config {
    // Reload from Cargo.toml if it has been modified on disk.
    pub fn reload_if_modified(&mut self) -> ConfigResult<bool> {
        let cargo_toml_path = self.project_dir.join("Cargo.toml");
        let metadata = fs::metadata(&cargo_toml_path).map_err(|e| e.to_string())?;

        if let Ok(modified) = metadata.modified() {
            if modified > self.last_modified {
                let new_config = build_config(&self.project_dir)?;
                self.environment = new_config.environment;
                self.project_name = new_config.project_name;
                self.show_compiler_warnings = new_config.show_compiler_warnings;
                self.last_modified = new_config.last_modified;
                return Ok(true);
            }
        }

        Ok(false)
    }

    // Toggle between dev and prod environment (in-memory only until Blueprint lands).
    pub fn toggle_environment(&mut self) -> Result<(), String> {
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

// Build a Config by reading Cargo.toml and environment hints.
fn build_config(project_dir: &Path) -> ConfigResult<Config> {
    let cargo_toml_path = project_dir.join("Cargo.toml");
    let cargo_str = fs::read_to_string(&cargo_toml_path).map_err(|e| e.to_string())?;
    let cargo: Value = toml::from_str(&cargo_str).map_err(|e| e.to_string())?;

    let project_name = cargo
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let environment = std::env::var("BLAST_ENV").unwrap_or_else(|_| "dev".to_string());

    let metadata = fs::metadata(&cargo_toml_path).map_err(|e| e.to_string())?;
    let last_modified = metadata.modified().map_err(|e| e.to_string())?;

    Ok(Config {
        environment,
        project_name,
        project_dir: project_dir.to_path_buf(),
        show_compiler_warnings: true,
        last_modified,
    })
}

// Load project configuration from the current directory.
pub fn get_project_info() -> ConfigResult<Config> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    build_config(&cwd)
}

// Force reload a fresh config from the project directory.
pub fn get_fresh_config(project_dir: &Path) -> ConfigResult<Config> {
    build_config(project_dir)
}
