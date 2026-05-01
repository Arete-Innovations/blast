use std::{collections::HashMap, process::Command};

use crate::error::{BlastError, BlastResult};

pub struct DependencyManager {
    install_hints: HashMap<String, String>,
    checked: HashMap<String, bool>,
}

impl DependencyManager {
    pub fn new() -> Self {
        let mut hints = HashMap::new();
        hints.insert("zellij".to_string(), "cargo install zellij".to_string());
        hints.insert("diesel_cli_ext".to_string(), "cargo install diesel_cli_ext".to_string());
        hints.insert("diesel".to_string(), "cargo install diesel_cli --no-default-features --features postgres".to_string());
        hints.insert("cargo-watch".to_string(), "cargo install cargo-watch".to_string());
        hints.insert("npm".to_string(), "install Node.js (e.g. pacman -S nodejs npm / apt install nodejs npm)".to_string());

        DependencyManager {
            install_hints: hints,
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
            Err(_e) => false, // allow: which-exec failure means not installed; caller hard-fails
        };

        self.checked.insert(name.to_string(), check_result);
        check_result
    }

    pub fn ensure_installed(&mut self, deps: &[&str]) -> BlastResult<()> {
        let mut missing = Vec::new();

        for &dep in deps {
            if !self.is_installed(dep) {
                missing.push(dep.to_string());
            }
        }

        if missing.is_empty() {
            return Ok(());
        }

        let lines: Vec<String> = missing
            .iter()
            .map(|name| match self.install_hints.get(name) {
                Some(cmd) => format!("  {}: {}", name, cmd),
                None => format!("  {}: <no install hint>", name),
            })
            .collect();

        Err(BlastError::MissingDep(format!("missing tooling — install before re-running:\n{}", lines.join("\n"))))
    }

    pub fn ensure_diesel_with_postgres_features(&mut self) -> BlastResult<()> {
        if !self.is_installed("diesel") {
            return self.ensure_installed(&["diesel"]);
        }

        let test_output = Command::new("diesel").args(["print-schema", "--database-url", "postgres://fake:fake@localhost/fake"]).output()?;
        let stderr = String::from_utf8_lossy(&test_output.stderr);

        if stderr.contains("requires the `postgres` feature but it's not enabled") {
            return Err(BlastError::MissingDep(
                "diesel_cli is installed without the postgres feature; reinstall: cargo install diesel_cli --no-default-features --features postgres --force".to_string(),
            ));
        }

        Ok(())
    }
}
