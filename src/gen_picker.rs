use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use crate::{
    commands::{Command, GenCmd},
    error::{BlastError, BlastResult},
};

pub fn pick_gen_target() -> BlastResult<Command> {
    let items = vec![
        "[GEN] Generate ALL (full pipeline)",
        "[GEN] Generate resource (TUI wizard)",
        "[GEN] Generate schema (diesel print-schema)",
        "[GEN] Generate structs",
        "[GEN] Generate models",
        "[GEN] Generate table (interactive wizard)",
        "[GEN] Generate custom migration (--custom)",
        "[GEN] Generate frontend (validators + list query helpers)",
        "[GEN] Generate governor Vite plugin (frontend/scripts/governor-plugin.js)",
        "[GEN] Generate test scaffolds (per-flow + per-route, idempotent)",
    ];

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default()).with_prompt("blast gen — pick a target").items(&items).default(0).interact()?;

    match selection {
        0 => Ok(Command::Gen { cmd: Some(GenCmd::All) }),
        1 => Ok(Command::Gen {
            cmd: Some(GenCmd::Resource { name: None }),
        }),
        2 => Ok(Command::Schema),
        3 => Ok(Command::Gen { cmd: Some(GenCmd::Structs) }),
        4 => Ok(Command::Gen { cmd: Some(GenCmd::Models) }),
        5 => Ok(Command::Gen { cmd: Some(GenCmd::Table) }),
        6 => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default()).with_prompt("Migration name (snake_case)").interact_text()?;
            Ok(Command::Gen {
                cmd: Some(GenCmd::Migration { custom: true, name: Some(name) }),
            })
        }
        7 => Ok(Command::Gen {
            cmd: Some(GenCmd::Types { resource: None }),
        }),
        8 => Ok(Command::Gen {
            cmd: Some(GenCmd::Api { resource: None }),
        }),
        9 => Ok(Command::Gen {
            cmd: Some(GenCmd::Pages { resource: None }),
        }),
        10 => Ok(Command::Gen { cmd: Some(GenCmd::GovernorPlugin) }),
        other => Err(BlastError::Invalid(format!("unknown gen picker selection: {}", other))),
    }
}
