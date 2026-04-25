
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use crate::commands::{execute, Command, GenTestSelector};
use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::error::{BlastError, BlastResult};

pub fn run(config: &mut Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    let items = vec![
        "Generate schema (diesel print-schema)",
        "Generate structs",
        "Generate models",
        "Generate table (interactive wizard)",
        "Generate custom migration (--custom)",
        "Generate frontend (validators + list query helpers)",
        "Generate governor Vite plugin (frontend/scripts/governor-plugin.js)",
        "Seed frontend scaffold (tokens.css, base.css, primevue.ts)",
        "Generate test scaffolds (per-flow + per-route, idempotent)",
    ];

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("blast gen — pick a target")
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => execute(Command::GenerateSchema, config, dep_manager),
        1 => execute(Command::GenerateStructs, config, dep_manager),
        2 => execute(Command::GenerateModels, config, dep_manager),
        3 => execute(Command::GenTable, config, dep_manager),
        4 => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Migration name (snake_case)")
                .interact_text()?;
            execute(Command::GenMigrationCustom(name), config, dep_manager)
        }
        5 => execute(Command::GenFrontend, config, dep_manager),
        6 => execute(Command::GenGovernorPlugin, config, dep_manager),
        7 => execute(Command::GenFeScaffold, config, dep_manager),
        8 => execute(
            Command::GenTest(GenTestSelector::All),
            config,
            dep_manager,
        ),
        other => Err(BlastError::Invalid(format!(
            "unknown gen picker selection: {}",
            other
        ))),
    }
}
