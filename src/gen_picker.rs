
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use crate::commands::{Command, GenCmd};
use crate::error::{BlastError, BlastResult};

pub fn pick_gen_target() -> BlastResult<Command> {
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
        0 => Ok(Command::Schema),
        1 => Ok(Command::Gen { cmd: Some(GenCmd::Structs) }),
        2 => Ok(Command::Gen { cmd: Some(GenCmd::Models) }),
        3 => Ok(Command::Gen { cmd: Some(GenCmd::Table) }),
        4 => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Migration name (snake_case)")
                .interact_text()?;
            Ok(Command::Gen {
                cmd: Some(GenCmd::Migration { custom: true, name: Some(name) }),
            })
        }
        5 => Ok(Command::Gen { cmd: Some(GenCmd::Frontend) }),
        6 => Ok(Command::Gen { cmd: Some(GenCmd::GovernorPlugin) }),
        7 => Ok(Command::Gen { cmd: Some(GenCmd::FeScaffold) }),
        8 => Ok(Command::Gen {
            cmd: Some(GenCmd::Test { flow: None, route: None }),
        }),
        other => Err(BlastError::Invalid(format!(
            "unknown gen picker selection: {}",
            other
        ))),
    }
}
