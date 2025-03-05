use std::collections::HashMap;
use std::error::Error;
use std::process::Command;

// Structure to hold dependency information
pub struct Dependency {
    pub name: String,
    pub install_command: String,
    #[allow(dead_code)]
    pub is_required: bool,
}

// Central registry of all dependencies used by the application
pub struct DependencyManager {
    dependencies: HashMap<String, Dependency>,
}

impl DependencyManager {
    // Create a new dependency manager with default dependencies
    pub fn new() -> Self {
        let mut deps = HashMap::new();

        // Add default dependencies
        deps.insert(
            "zellij".to_string(),
            Dependency {
                name: "zellij".to_string(),
                install_command: "cargo install zellij".to_string(),
                is_required: true,
            },
        );

        deps.insert(
            "diesel".to_string(),
            Dependency {
                name: "diesel_ext".to_string(),
                install_command: "cargo install diesel_cli_ext".to_string(),
                is_required: true,
            },
        );
        deps.insert(
            "diesel".to_string(),
            Dependency {
                name: "diesel".to_string(),
                install_command: "cargo install diesel_cli --no-default-features --features postgres".to_string(),
                is_required: true,
            },
        );

        DependencyManager { dependencies: deps }
    }

    // Register a new dependency
    #[allow(dead_code)]
    pub fn register(&mut self, name: &str, install_command: &str, is_required: bool) {
        self.dependencies.insert(
            name.to_string(),
            Dependency {
                name: name.to_string(),
                install_command: install_command.to_string(),
                is_required,
            },
        );
    }

    // Check if a dependency is installed
    pub fn is_installed(&self, name: &str) -> bool {
        let output = Command::new("which").arg(name).output();

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    // Get missing dependencies
    #[allow(dead_code)]
    pub fn get_missing(&self, required_only: bool) -> Vec<&Dependency> {
        self.dependencies.values().filter(|dep| (!required_only || dep.is_required) && !self.is_installed(&dep.name)).collect()
    }

    // Check a specific set of dependencies
    pub fn check_specific(&self, dep_names: &[&str]) -> Vec<&Dependency> {
        dep_names
            .iter()
            .filter_map(|&name| {
                if let Some(dep) = self.dependencies.get(name) {
                    if !self.is_installed(name) {
                        return Some(dep);
                    }
                }
                None
            })
            .collect()
    }

    // Install a specific dependency
    pub fn install(&self, name: &str) -> Result<(), Box<dyn Error>> {
        if let Some(dep) = self.dependencies.get(name) {
            // Split the install command into program and args
            let parts: Vec<&str> = dep.install_command.split_whitespace().collect();
            if parts.is_empty() {
                return Err(format!("Invalid install command for {}", name).into());
            }

            let program = parts[0];
            let args = &parts[1..];

            let status = Command::new(program).args(args).status()?;

            if !status.success() {
                return Err(format!("Failed to install {}", name).into());
            }

            Ok(())
        } else {
            Err(format!("Dependency {} not found in registry", name).into())
        }
    }

    // Ensure a set of dependencies are installed
    pub fn ensure_installed(&self, dep_names: &[&str], silent: bool) -> Result<bool, Box<dyn Error>> {
        let missing = self.check_specific(dep_names);

        if missing.is_empty() {
            return Ok(true);
        }

        if !silent {
            println!("Missing required dependencies:");
            for dep in &missing {
                println!("  - {}: Install with '{}'", dep.name, dep.install_command);
            }
        }

        for dep in missing {
            self.install(&dep.name)?;
        }

        Ok(true)
    }
}
