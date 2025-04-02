use crate::logger;
use dialoguer::Confirm;
use std::collections::HashMap;
use std::error::Error;
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
        deps.insert("diesel".to_string(), "cargo install diesel_cli --no-default-features --features postgres".to_string());
        deps.insert("diesel_ext".to_string(), "cargo install diesel_ext".to_string());

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
    pub fn ensure_installed(&mut self, deps: &[&str], prompt: bool) -> Result<(), Box<dyn Error>> {
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
            let confirm = Confirm::new().with_prompt(format!("Missing dependencies: {}. Install now?", deps_list)).default(true).interact()?;

            if !confirm {
                return Err(format!("Required dependencies not installed: {}", deps_list).into());
            }
        }

        // Install missing dependencies
        for dep in missing {
            self.install_dependency(dep)?;
        }

        Ok(())
    }

    // Install a specific dependency
    fn install_dependency(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if let Some(install_cmd) = self.dependencies.get(name) {
            let mut progress = logger::create_progress(None);
            progress.set_message(&format!("Installing {}...", name));

            // Split the install command into program and args
            let parts: Vec<&str> = install_cmd.split_whitespace().collect();
            if parts.is_empty() {
                return Err(format!("Invalid install command for {}", name).into());
            }

            let program = parts[0];
            let args = &parts[1..];

            // Run the installation command
            let status = Command::new(program).args(args).status()?;

            if status.success() {
                // Mark as installed in the cache
                self.checked.insert(name.to_string(), true);
                progress.success(&format!("{} installed successfully", name));
                Ok(())
            } else {
                progress.error(&format!("Failed to install {}", name));
                Err(format!("Failed to install {}", name).into())
            }
        } else {
            Err(format!("No installer found for dependency: {}", name).into())
        }
    }

    // Register a new dependency
    #[allow(dead_code)]
    pub fn register(&mut self, name: &str, install_command: &str) {
        self.dependencies.insert(name.to_string(), install_command.to_string());
    }
}
