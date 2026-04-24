use crate::error::{BlastError, BlastResult};
use crate::logger;
use dialoguer::Confirm;
use std::collections::HashMap;
use std::process::Command;

pub struct DependencyManager {
    dependencies: HashMap<String, String>,
    checked: HashMap<String, bool>,
}

impl DependencyManager {
    pub fn new() -> Self {
        let mut deps = HashMap::new();

        deps.insert("zellij".to_string(), "cargo install zellij".to_string());
        deps.insert("diesel_cli_ext".to_string(), "cargo install diesel_cli_ext".to_string());
        deps.insert("diesel".to_string(), "cargo install diesel_cli --no-default-features --features postgres".to_string());
        deps.insert("cargo-watch".to_string(), "cargo install cargo-watch".to_string());

        DependencyManager {
            dependencies: deps,
            checked: HashMap::new(),
        }
    }

    pub fn is_installed(&mut self, name: &str) -> bool {
        match self.checked.get(name) {
            Some(installed) => return *installed,
            None => {}
        }

        let check_result = match Command::new("which").arg(name).output() {
            Ok(output) => output.status.success(),
            Err(_e) => {
                false
            }
        };

        self.checked.insert(name.to_string(), check_result);
        check_result
    }

    pub fn ensure_installed(&mut self, deps: &[&str], prompt: bool) -> BlastResult<()> {
        let mut missing = Vec::new();

        for &dep in deps {
            if !self.is_installed(dep) {
                missing.push(dep);
            }
        }

        if missing.is_empty() {
            return Ok(());
        }

        if prompt {
            let deps_list = missing.join(", ");
            let confirm = Confirm::new()
                .with_prompt(format!("Missing dependencies: {}. Install now?", deps_list))
                .default(true)
                .interact()?;

            if !confirm {
                return Err(BlastError::MissingDep(deps_list));
            }
        }

        for dep in missing {
            self.install_dependency(dep)?;
        }

        Ok(())
    }

    fn install_dependency(&mut self, name: &str) -> BlastResult<()> {
        let install_cmd = match self.dependencies.get(name) {
            Some(cmd) => cmd.clone(),
            None => return Err(BlastError::MissingDep(format!("no installer for {}", name))),
        };

        let mut progress = logger::create_progress(None);
        progress.set_message(&format!("Installing {}...", name));

        let parts: Vec<&str> = install_cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Err(BlastError::Invalid(format!("invalid install command for {}", name)));
        }

        let program = parts[0];
        let args = &parts[1..];

        let status = if name == "diesel" {
            let mut cmd = Command::new(program);
            cmd.args(args);
            cmd.arg("--force");
            cmd.arg("--quiet");
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
            cmd.status()?
        } else {
            Command::new(program).args(args).status()?
        };

        if status.success() {
            self.checked.insert(name.to_string(), true);
            progress.success(&format!("{} installed successfully", name));
            Ok(())
        } else {
            progress.error(&format!("Failed to install {}", name));
            Err(BlastError::Subprocess {
                cmd: install_cmd,
                detail: format!("install of {} exited with {}", name, status),
            })
        }
    }

    pub fn ensure_diesel_with_postgres_features(&mut self) -> BlastResult<()> {
        if self.is_installed("diesel") {
            let output = Command::new("diesel")
                .args(["--version"])
                .output()?;

            if output.status.success() {
                let test_output = Command::new("diesel")
                    .args(["print-schema", "--database-url", "postgres://fake:fake@localhost/fake"])
                    .output()?;

                let stderr = String::from_utf8_lossy(&test_output.stderr);

                if stderr.contains("requires the `postgres` feature but it's not enabled") {
                    let mut progress = logger::create_progress(None);
                    progress.set_message("Installing diesel_cli with PostgreSQL support...");

                    let install_status = Command::new("cargo")
                        .args(["install", "diesel_cli", "--no-default-features", "--features", "postgres", "--force", "--quiet"])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()?;

                    if install_status.success() {
                        self.checked.insert("diesel".to_string(), true);
                        progress.success("diesel_cli installed with PostgreSQL support");
                        return Ok(());
                    } else {
                        progress.error("Failed to install diesel_cli with PostgreSQL features");
                        return Err(BlastError::Subprocess {
                            cmd: "cargo install diesel_cli".to_string(),
                            detail: format!("exited with {}", install_status),
                        });
                    }
                }

                return Ok(());
            }
        }

        self.install_dependency("diesel")
    }
}
