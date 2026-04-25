
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use crate::commands::{execute, Command, GenCmd};
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

    let target = match selection {
        0 => Command::Schema,
        1 => Command::Gen { cmd: Some(GenCmd::Structs) },
        2 => Command::Gen { cmd: Some(GenCmd::Models) },
        3 => Command::Gen { cmd: Some(GenCmd::Table) },
        4 => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Migration name (snake_case)")
                .interact_text()?;
            Command::Gen {
                cmd: Some(GenCmd::Migration { custom: true, name: Some(name) }),
            }
        }
        5 => Command::Gen { cmd: Some(GenCmd::Frontend) },
        6 => Command::Gen { cmd: Some(GenCmd::GovernorPlugin) },
        7 => Command::Gen { cmd: Some(GenCmd::FeScaffold) },
        8 => Command::Gen {
            cmd: Some(GenCmd::Test { flow: None, route: None }),
        },
        other => return Err(BlastError::Invalid(format!(
            "unknown gen picker selection: {}",
            other
        ))),
    };
    execute(target, config, dep_manager)
}
