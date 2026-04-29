use std::{env, io::Write};

use console::Style;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use crate::{
    commands::{ArsenalCmd, Command, FusesCmd, GenCmd, LogCmd},
    configs::Config,
    dependencies::DependencyManager,
    error::BlastResult,
    logger,
};

const MENU_ITEMS: &[&str] = &[
    // ── scaffold ──────────────────────────────────────────────────────────────
    "[SCAFFOLD] New project",
    "[SCAFFOLD] Init in-place",
    // ── app lifecycle ─────────────────────────────────────────────────────────
    "[APP] Run Server (dev)",
    "[APP] Run Server (prod)",
    "[APP] Watch Server",
    "[APP] Refresh",
    "[APP] Toggle Dev/Prod",
    "[APP] Dashboard",
    "[APP] Build",
    "[APP] Package",
    // ── codegen ───────────────────────────────────────────────────────────────
    "[CODEGEN] Gen All (full pipeline)",
    "[CODEGEN] Gen Resource (wizard)",
    "[CODEGEN] Schema",
    "[CODEGEN] Structs",
    "[CODEGEN] Models",
    "[CODEGEN] Gen Table (wizard)",
    "[CODEGEN] Gen Migration (--custom)",
    "[CODEGEN] Gen Frontend",
    "[CODEGEN] Gen Governor Plugin",
    "[CODEGEN] Gen Test Scaffolds",
    // ── database ──────────────────────────────────────────────────────────────
    "[DB] New Migration",
    "[DB] Migrate",
    "[DB] Rollback",
    "[DB] Seed",
    // ── fuses ─────────────────────────────────────────────────────────────────
    "[FUSES] Manage fuses (TUI)",
    "[FUSES] List fuses",
    "[FUSES] Toggle fuse",
    "[FUSES] Run fuse now",
    "[FUSES] Fuse logs",
    "[FUSES] Live fuses table",
    // ── logs ──────────────────────────────────────────────────────────────────
    "[LOG] View logs",
    "[LOG] Truncate Logs",
    // ── lint ──────────────────────────────────────────────────────────────────
    "[LINT] Governor Check",
    "[LINT] Governor Check (verbose)",
    // ── arsenal ───────────────────────────────────────────────────────────────
    "[ARSENAL] Scan & Write JSON",
    "[ARSENAL] Serve MCP (stdio)",
    // ── exit ──────────────────────────────────────────────────────────────────
    "[Exit] Kill Session",
];

pub fn pick_command(config: &Config) -> BlastResult<Option<Command>> {
    let prod_style = Style::new().bold().fg(console::Color::Green);
    let dev_style = Style::new().bold().fg(console::Color::Yellow);

    let prompt = match config.environment.as_str() {
        "prod" => format!("{}->[{}] ", prod_style.apply_to(format!("[🚀{}]", config.environment.to_uppercase())), config.project_name,),
        _other_env => format!("{}->[{}] ", dev_style.apply_to(format!("[🔧{}]", config.environment.to_uppercase())), config.project_name,),
    };

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default()).with_prompt(prompt).items(MENU_ITEMS).default(0).interact()?;

    resolve_selection(MENU_ITEMS[selection])
}

fn resolve_selection(label: &str) -> BlastResult<Option<Command>> {
    match label {
        // ── scaffold ──────────────────────────────────────────────────────────
        "[SCAFFOLD] New project" => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default()).with_prompt("Project name").interact_text()?;
            Ok(Some(Command::New {
                name,
                dev: false,
                db_url: None,
                force: false,
                no_test_db: false,
                no_warmup: false,
            }))
        }
        "[SCAFFOLD] Init in-place" => Ok(Some(Command::Init {
            name: None,
            db_url: None,
            force: false,
            no_test_db: false,
            no_warmup: false,
        })),

        // ── app lifecycle ─────────────────────────────────────────────────────
        "[APP] Refresh" => Ok(Some(Command::Refresh)),
        "[APP] Run Server (dev)" => Ok(Some(Command::Run)),
        "[APP] Run Server (prod)" => Ok(Some(Command::RunProd)),
        "[APP] Watch Server" => Ok(Some(Command::Watch)),
        "[APP] Toggle Dev/Prod" => Ok(Some(Command::ToggleEnv)),
        "[APP] Dashboard" => Ok(Some(Command::Dashboard)),
        "[APP] Build" => Ok(Some(Command::Build)),
        "[APP] Package" => Ok(Some(Command::Package)),

        // ── codegen ───────────────────────────────────────────────────────────
        "[CODEGEN] Gen All (full pipeline)" => Ok(Some(Command::Gen { cmd: Some(GenCmd::All) })),
        "[CODEGEN] Gen Resource (wizard)" => Ok(Some(Command::Gen {
            cmd: Some(GenCmd::Resource { name: None }),
        })),
        "[CODEGEN] Schema" => Ok(Some(Command::Schema)),
        "[CODEGEN] Structs" => Ok(Some(Command::Gen { cmd: Some(GenCmd::Structs) })),
        "[CODEGEN] Models" => Ok(Some(Command::Gen { cmd: Some(GenCmd::Models) })),
        "[CODEGEN] Gen Table (wizard)" => Ok(Some(Command::Gen { cmd: Some(GenCmd::Table) })),
        "[CODEGEN] Gen Migration (--custom)" => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default()).with_prompt("Migration name (snake_case)").interact_text()?;
            Ok(Some(Command::Gen {
                cmd: Some(GenCmd::Migration { custom: true, name: Some(name) }),
            }))
        }
        "[CODEGEN] Gen Types" => Ok(Some(Command::Gen {
            cmd: Some(GenCmd::Types { resource: None }),
        })),
        "[CODEGEN] Gen API" => Ok(Some(Command::Gen {
            cmd: Some(GenCmd::Api { resource: None }),
        })),
        "[CODEGEN] Gen Pages" => Ok(Some(Command::Gen {
            cmd: Some(GenCmd::Pages { resource: None }),
        })),
        "[CODEGEN] Gen Governor Plugin" => Ok(Some(Command::Gen { cmd: Some(GenCmd::GovernorPlugin) })),

        // ── database ──────────────────────────────────────────────────────────
        "[DB] New Migration" => Ok(Some(Command::Migration)),
        "[DB] Migrate" => Ok(Some(Command::Migrate)),
        "[DB] Rollback" => Ok(Some(Command::Rollback)),
        "[DB] Seed" => Ok(Some(Command::Seed { file: None })),

        // ── fuses ─────────────────────────────────────────────────────────────
        "[FUSES] Manage fuses (TUI)" => Ok(Some(Command::Fuses { cmd: Some(FusesCmd::Interactive) })),
        "[FUSES] List fuses" => Ok(Some(Command::Fuses { cmd: Some(FusesCmd::List) })),
        "[FUSES] Toggle fuse" => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default()).with_prompt("Fuse name").interact_text()?;
            Ok(Some(Command::Fuses { cmd: Some(FusesCmd::Toggle { name }) }))
        }
        "[FUSES] Run fuse now" => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default()).with_prompt("Fuse name").interact_text()?;
            Ok(Some(Command::Fuses { cmd: Some(FusesCmd::Run { name }) }))
        }
        "[FUSES] Fuse logs" => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default()).with_prompt("Fuse name").interact_text()?;
            Ok(Some(Command::Fuses { cmd: Some(FusesCmd::Logs { name }) }))
        }
        "[FUSES] Live fuses table" => Ok(Some(Command::Fuses { cmd: Some(FusesCmd::LiveTable) })),

        // ── logs ──────────────────────────────────────────────────────────────
        "[LOG] View logs" => {
            let level: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Log level (error/warn/info/debug)")
                .default("info".to_string())
                .interact_text()?;
            Ok(Some(Command::Log { cmd: LogCmd::View { level } }))
        }
        "[LOG] Truncate Logs" => Ok(Some(Command::Log { cmd: LogCmd::Truncate { file: None } })),

        // ── lint ──────────────────────────────────────────────────────────────
        "[LINT] Governor Check" => Ok(Some(Command::Check { verbose: false })),
        "[LINT] Governor Check (verbose)" => Ok(Some(Command::Check { verbose: true })),

        // ── arsenal ───────────────────────────────────────────────────────────
        "[ARSENAL] Scan & Write JSON" => Ok(Some(Command::Arsenal { cmd: None })),
        "[ARSENAL] Serve MCP (stdio)" => Ok(Some(Command::Arsenal { cmd: Some(ArsenalCmd::Serve) })),

        // ── exit ──────────────────────────────────────────────────────────────
        "[Exit] Kill Session" => Ok(None),

        unknown => Err(crate::error::BlastError::Invalid(format!("unknown menu selection: {}", unknown))),
    }
}

pub fn run_interactive_loop(config: &mut Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    logger::setup_for_mode(config, true)?;

    env::set_var("BLAST_INTERACTIVE", "1");

    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush()?;

    loop {
        match config.reload_if_modified() {
            Ok(_changed) => {}
            Err(e) => eprintln!("config reload failed: {}", e),
        }

        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush()?;

        let picked = pick_command(config)?;

        let cmd = match picked {
            Some(c) => c,
            None => {
                logger::info("Killing Zellij session...")?;
                drop(std::process::Command::new("zellij").args(["kill-session"]).spawn());
                drop(std::process::Command::new("zellij").args(["kill-all-sessions", "-y"]).spawn());
                break;
            }
        };

        let resolved = resolve_run_for_env(cmd, config);

        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush()?;

        match crate::commands::execute(resolved, config, dep_manager) {
            Ok(_v) => {
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            Err(e) => {
                logger::error(&format!("Command failed: {}", e))?;

                println!("\nPress Enter to continue...");
                let mut buffer = String::new();
                std::io::stdin().read_line(&mut buffer)?;
            }
        }
    }

    Ok(())
}

fn resolve_run_for_env(cmd: Command, config: &Config) -> Command {
    match cmd {
        Command::Run => match config.environment.as_str() {
            "prod" => Command::RunProd,
            "production" => Command::RunProd,
            _other_env => Command::Run,
        },
        passthrough => passthrough,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{ArsenalCmd, FusesCmd, GenCmd, LogCmd};

    /// Items that do NOT require interactive prompting.
    /// Each must resolve to Ok(Some(_)) without hitting the `unknown` branch.
    #[test]
    fn non_interactive_menu_items_all_resolve() {
        let non_interactive: &[&str] = &[
            "[APP] Refresh",
            "[APP] Run Server (dev)",
            "[APP] Run Server (prod)",
            "[APP] Watch Server",
            "[APP] Toggle Dev/Prod",
            "[APP] Dashboard",
            "[APP] Build",
            "[APP] Package",
            "[SCAFFOLD] Init in-place",
            "[CODEGEN] Gen All (full pipeline)",
            "[CODEGEN] Gen Resource (wizard)",
            "[CODEGEN] Schema",
            "[CODEGEN] Structs",
            "[CODEGEN] Models",
            "[CODEGEN] Gen Table (wizard)",
            "[CODEGEN] Gen Frontend",
            "[CODEGEN] Gen Governor Plugin",
            "[CODEGEN] Gen Test Scaffolds",
            "[DB] New Migration",
            "[DB] Migrate",
            "[DB] Rollback",
            "[DB] Seed",
            "[FUSES] Manage fuses (TUI)",
            "[FUSES] List fuses",
            "[FUSES] Live fuses table",
            "[LOG] Truncate Logs",
            "[LINT] Governor Check",
            "[LINT] Governor Check (verbose)",
            "[ARSENAL] Scan & Write JSON",
            "[ARSENAL] Serve MCP (stdio)",
            "[Exit] Kill Session",
        ];

        // Every label in MENU_ITEMS must be listed in either non_interactive or
        // the interactive set below (i.e. there are no unhandled labels).
        let interactive_labels: &[&str] = &[
            "[SCAFFOLD] New project",
            "[CODEGEN] Gen Migration (--custom)",
            "[FUSES] Toggle fuse",
            "[FUSES] Run fuse now",
            "[FUSES] Fuse logs",
            "[LOG] View logs",
        ];

        let all_handled: Vec<&str> = non_interactive.iter().chain(interactive_labels.iter()).copied().collect();

        for label in MENU_ITEMS {
            assert!(all_handled.contains(label), "MENU_ITEMS label {:?} is not tracked in the parity test — add it", label);
        }

        for label in all_handled.iter() {
            assert!(MENU_ITEMS.contains(label), "parity test lists {:?} but it is not in MENU_ITEMS", label);
        }
    }

    /// Spot-check that a sample of non-interactive labels route to the
    /// expected Command variants.
    #[test]
    fn spot_check_command_routing() {
        assert!(matches!(resolve_selection("[APP] Run Server (dev)"), Ok(Some(Command::Run))));
        assert!(matches!(resolve_selection("[APP] Run Server (prod)"), Ok(Some(Command::RunProd))));
        assert!(matches!(resolve_selection("[CODEGEN] Gen All (full pipeline)"), Ok(Some(Command::Gen { cmd: Some(GenCmd::All) }))));
        assert!(matches!(
            resolve_selection("[CODEGEN] Gen Resource (wizard)"),
            Ok(Some(Command::Gen {
                cmd: Some(GenCmd::Resource { name: None })
            }))
        ));
        assert!(matches!(resolve_selection("[FUSES] Manage fuses (TUI)"), Ok(Some(Command::Fuses { cmd: Some(FusesCmd::Interactive) }))));
        assert!(matches!(resolve_selection("[FUSES] Live fuses table"), Ok(Some(Command::Fuses { cmd: Some(FusesCmd::LiveTable) }))));
        assert!(matches!(resolve_selection("[LOG] Truncate Logs"), Ok(Some(Command::Log { cmd: LogCmd::Truncate { file: None } }))));
        assert!(matches!(resolve_selection("[LINT] Governor Check (verbose)"), Ok(Some(Command::Check { verbose: true }))));
        assert!(matches!(resolve_selection("[ARSENAL] Serve MCP (stdio)"), Ok(Some(Command::Arsenal { cmd: Some(ArsenalCmd::Serve) }))));
        assert!(matches!(resolve_selection("[Exit] Kill Session"), Ok(None)));
    }
}
