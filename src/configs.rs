use crate::logger;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use toml::Value;

// Type alias for consistent error handling
type BlastResult = Result<(), String>;
type ConfigResult<T> = Result<T, String>;

#[derive(Clone, Debug)]
pub struct Config {
    pub environment: String,
    pub project_name: String,
    pub assets: Value,         // This is the raw TOML data
    pub project_dir: PathBuf,
    pub show_compiler_warnings: bool,
    pub last_modified: SystemTime,
}

impl Config {
    // Create a config from path
    pub fn from_path(config_path: &Path, project_dir: &Path) -> ConfigResult<Self> {
        let config_str = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
        let config_val: Value = config_str.parse::<Value>().map_err(|e| e.to_string())?;
        let metadata = fs::metadata(config_path).map_err(|e| e.to_string())?;

        let cargo_toml_path = project_dir.join("Cargo.toml");
        let cargo_str = fs::read_to_string(&cargo_toml_path).map_err(|e| e.to_string())?;
        let cargo: Value = cargo_str.parse::<Value>().map_err(|e| e.to_string())?;
        let project_name = cargo.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("Unknown").to_string();

        // Get environment from TOML or default to dev
        let environment = config_val.get("settings").and_then(|s| s.get("environment")).and_then(|e| e.as_str()).unwrap_or("dev").to_string();

        // Get show_compiler_warnings from TOML or default to true
        let show_compiler_warnings = config_val.get("settings").and_then(|s| s.get("show_compiler_warnings")).and_then(|v| v.as_bool()).unwrap_or(true);

        Ok(Config {
            environment,
            project_name,
            assets: config_val,
            project_dir: project_dir.to_path_buf(),
            show_compiler_warnings,
            last_modified: metadata.modified().map_err(|e| e.to_string())?,
        })
    }

    // Check if config file has been modified and reload if necessary
    pub fn reload_if_modified(&mut self) -> ConfigResult<bool> {
        let config_path = self.project_dir.join("Catalyst.toml");
        let metadata = fs::metadata(&config_path).map_err(|e| e.to_string())?;

        if let Ok(modified) = metadata.modified() {
            if modified > self.last_modified {
                // File has been modified, reload
                logger::debug("Catalyst.toml modified, reloading configuration")?;

                let new_config = Self::from_path(&config_path, &self.project_dir)?;

                // Update this config with new values
                self.environment = new_config.environment;
                self.project_name = new_config.project_name;
                self.assets = new_config.assets;
                self.show_compiler_warnings = new_config.show_compiler_warnings;
                self.last_modified = new_config.last_modified;

                return Ok(true);
            }
        }

        Ok(false)
    }

    // Helper method to update a setting in the TOML file
    fn update_setting<T: Into<toml::Value>>(&mut self, key: &str, value: T) -> BlastResult {
        // Update the config file
        let config_path = self.project_dir.join("Catalyst.toml");
        let toml_content = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;

        // Parse the TOML content
        let mut parsed_toml: toml::Value = toml_content.parse::<toml::Value>().map_err(|e| e.to_string())?;

        // Make sure the settings table exists
        if !parsed_toml.as_table().unwrap().contains_key("settings") {
            parsed_toml.as_table_mut().unwrap().insert("settings".to_string(), toml::Value::Table(toml::value::Table::new()));
        }

        // Update the setting
        parsed_toml.as_table_mut().unwrap().get_mut("settings").unwrap().as_table_mut().unwrap().insert(key.to_string(), value.into());

        // Format the TOML content and write it back
        let formatted_toml = toml::to_string_pretty(&parsed_toml).map_err(|e| e.to_string())?;
        fs::write(&config_path, formatted_toml).map_err(|e| e.to_string())?;

        // Update last_modified and assets
        if let Ok(metadata) = fs::metadata(&config_path) {
            if let Ok(modified) = metadata.modified() {
                self.last_modified = modified;
                self.assets = parsed_toml;
            }
        }

        Ok(())
    }

    // Toggle between dev and prod environment
    pub fn toggle_environment(&mut self) -> Result<(), String> {
        let old_env = self.environment.clone();

        // Toggle environment
        self.environment = if self.environment == "dev" { "prod".to_string() } else { "dev".to_string() };

        // Update the setting
        self.update_setting("environment", self.environment.clone())?;

        logger::success(&format!("Environment toggled from {} to {}", old_env, self.environment))?;
        Ok(())
    }

    // Toggle compiler warnings
    #[allow(dead_code)]
    pub fn toggle_compiler_warnings(&mut self) -> Result<(), String> {
        let old_state = self.show_compiler_warnings;
        let old_state_str = if old_state { "enabled" } else { "disabled" };

        // Toggle show_compiler_warnings
        self.show_compiler_warnings = !self.show_compiler_warnings;
        let new_state_str = if self.show_compiler_warnings { "enabled" } else { "disabled" };

        // Update the setting
        self.update_setting("show_compiler_warnings", self.show_compiler_warnings)?;

        logger::success(&format!("Compiler warnings changed from {} to {}", old_state_str, new_state_str))?;
        Ok(())
    }
}

// Load project configuration from the current directory
pub fn get_project_info() -> ConfigResult<Config> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let config_path = cwd.join("Catalyst.toml");
    Config::from_path(&config_path, &cwd)
}

// Load project configuration from a specific path
#[allow(dead_code)]
pub fn get_project_info_with_paths(config_path: &str, project_dir: &Path) -> ConfigResult<Config> {
    Config::from_path(Path::new(config_path), project_dir)
}

// Force reload a fresh config from the project directory
pub fn get_fresh_config(project_dir: &Path) -> ConfigResult<Config> {
    let config_path = project_dir.join("Catalyst.toml");
    Config::from_path(&config_path, project_dir)
}