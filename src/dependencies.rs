use crate::logger;
use dialoguer::Confirm;
use std::collections::HashMap;
use std::process::Command;

// A centralized dependency manager for all external tools
pub struct DependencyManager {
    // Map of dependency name to install command
    dependencies: HashMap<String, String>,
    // Cache of already checked dependencies
    checked: HashMap<String, bool>,
}

impl DependencyManager {
    // Create a new dependency manager with pre-configured dependencies
    pub fn new() -> Self {
        let mut deps = HashMap::new();

        // Register known dependencies with their installation commands
        deps.insert("zellij".to_string(), "cargo install zellij".to_string());
        deps.insert("diesel_cli_ext".to_string(), "cargo install diesel_cli_ext".to_string());
        deps.insert("diesel".to_string(), "cargo install diesel_cli --no-default-features --features postgres".to_string());
        deps.insert("cargo-watch".to_string(), "cargo install cargo-watch".to_string());

        DependencyManager {
            dependencies: deps,
            checked: HashMap::new(),
        }
    }

    // Check if a dependency is installed
    pub fn is_installed(&mut self, name: &str) -> bool {
        // Check if we've already checked this dependency
        if let Some(installed) = self.checked.get(name) {
            return *installed;
        }

        // Run "which" command to check if tool is available
        let output = Command::new("which").arg(name).output();
        let check_result = match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        };

        // Cache the result
        self.checked.insert(name.to_string(), check_result);
        check_result
    }

    // Ensure that a dependency is installed
    pub fn ensure_installed(&mut self, deps: &[&str], prompt: bool) -> Result<(), String> {
        let mut missing = Vec::new();

        // Find missing dependencies
        for &dep in deps {
            if !self.is_installed(dep) {
                missing.push(dep);
            }
        }

        // Return early if all dependencies are installed
        if missing.is_empty() {
            return Ok(());
        }

        // In prompt mode, ask user before installing
        if prompt {
            let deps_list = missing.join(", ");
            let confirm = Confirm::new()
                .with_prompt(format!("Missing dependencies: {}. Install now?", deps_list))
                .default(true)
                .interact()
                .map_err(|e| e.to_string())?;

            if !confirm {
                return Err(format!("Required dependencies not installed: {}", deps_list));
            }
        }

        // Install missing dependencies
        for dep in missing {
            self.install_dependency(dep)?;
        }

        Ok(())
    }

    // Install a specific dependency
    fn install_dependency(&mut self, name: &str) -> Result<(), String> {
        if let Some(install_cmd) = self.dependencies.get(name) {
            // Always show progress indicator
            let mut progress = logger::create_progress(None);
            progress.set_message(&format!("Installing {}...", name));

            // Split the install command into program and args
            let parts: Vec<&str> = install_cmd.split_whitespace().collect();
            if parts.is_empty() {
                return Err(format!("Invalid install command for {}", name));
            }

            let program = parts[0];
            let args = &parts[1..];

            // Special case for diesel to install and hide cargo output but with --force to ensure correct features
            let status = if name == "diesel" {
                let mut cmd = Command::new(program);
                cmd.args(args);
                cmd.arg("--force");
                cmd.arg("--quiet"); // Add --quiet flag to reduce output
                cmd.stdout(std::process::Stdio::null());
                cmd.stderr(std::process::Stdio::null());
                cmd.status().map_err(|e| e.to_string())?
            } else {
                Command::new(program).args(args).status().map_err(|e| e.to_string())?
            };

            if status.success() {
                // Mark as installed in the cache
                self.checked.insert(name.to_string(), true);
                progress.success(&format!("{} installed successfully", name));
                Ok(())
            } else {
                progress.error(&format!("Failed to install {}", name));
                Err(format!("Failed to install {}", name))
            }
        } else {
            Err(format!("No installer found for dependency: {}", name))
        }
    }

    // Register a new dependency
    #[allow(dead_code)]
    pub fn register(&mut self, name: &str, install_command: &str) {
        self.dependencies.insert(name.to_string(), install_command.to_string());
    }

    // Check if diesel_cli is installed with PostgreSQL features
    // Silently fixes the installation if it's installed without PostgreSQL features
    pub fn ensure_diesel_with_postgres_features(&mut self) -> Result<(), String> {
        // First, check if diesel is installed at all
        if self.is_installed("diesel") {
            // Now check if it's installed with PostgreSQL features
            let output = Command::new("diesel")
                .args(["--version"])
                .output()
                .map_err(|_| "Failed to execute diesel --version".to_string())?;
            
            if output.status.success() {
                // Try a simple PostgreSQL test command
                let test_output = Command::new("diesel")
                    .args(["print-schema", "--database-url", "postgres://fake:fake@localhost/fake"])
                    .output()
                    .map_err(|_| "Failed to test diesel PostgreSQL features".to_string())?;
                
                let stderr = String::from_utf8_lossy(&test_output.stderr);
                
                // If stderr contains a message about missing PostgreSQL feature, reinstall with the feature
                if stderr.contains("requires the `postgres` feature but it's not enabled") {
                    // Install diesel_cli with PostgreSQL features
                    // Show the loading spinner but suppress the cargo output
                    let mut progress = logger::create_progress(None);
                    progress.set_message("Installing diesel_cli with PostgreSQL support...");
                    
                    let install_status = Command::new("cargo")
                        .args(["install", "diesel_cli", "--no-default-features", "--features", "postgres", "--force", "--quiet"])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map_err(|e| e.to_string())?;
                        
                    if install_status.success() {
                        // Mark as installed in the cache
                        self.checked.insert("diesel".to_string(), true);
                        progress.success("diesel_cli installed with PostgreSQL support");
                        return Ok(());
                    } else {
                        progress.error("Failed to install diesel_cli with PostgreSQL features");
                        return Err("Failed to install diesel_cli with PostgreSQL features".to_string());
                    }
                }
                
                // PostgreSQL feature is already available
                return Ok(());
            }
        }
        
        // Diesel is not installed, so install it with PostgreSQL features
        self.install_dependency("diesel")
    }
}
