use crate::commands::{Command, FusesCmd, GenCmd, LogCmd};
use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::error::BlastResult;
use crate::logger;
use console::Style;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use std::env;
use std::io::Write;

const MENU_ITEMS: &[&str] = &[
    "[APP] Run Server",
    "[APP] Watch Server",
    "[APP] Stop Server",
    "[APP] Refresh",
    "[APP] Toggle Dev/Prod",
    "[CODEGEN] Schema",
    "[CODEGEN] Structs",
    "[CODEGEN] Models",
    "[CODEGEN] Gen Picker",
    "[CODEGEN] Gen Table (wizard)",
    "[CODEGEN] Gen Migration (--custom)",
    "[CODEGEN] Gen Frontend",
    "[CODEGEN] Gen Governor Plugin",
    "[CODEGEN] Gen Test Scaffolds",
    "[DB] New Migration",
    "[DB] Migrate",
    "[DB] Rollback",
    "[DB] Seed",
    "[FUSES] Manage fuses",
    "[FUSES] List fuses",
    "[LOG] Truncate Logs",
    "[LINT] Governor Check",
    "[Arsenal] Scan & Write JSON",
    "[Exit] Kill Session",
];

pub fn pick_command(config: &Config) -> BlastResult<Option<Command>> {
    let prod_style = Style::new().bold().fg(console::Color::Green);
    let dev_style = Style::new().bold().fg(console::Color::Yellow);

    let prompt = match config.environment.as_str() {
        "prod" => format!(
            "{}->[{}] ",
            prod_style.apply_to(format!("[🚀{}]", config.environment.to_uppercase())),
            config.project_name,
        ),
        _other_env => format!(
            "{}->[{}] ",
            dev_style.apply_to(format!("[🔧{}]", config.environment.to_uppercase())),
            config.project_name,
        ),
    };

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(MENU_ITEMS)
        .default(0)
        .interact()?;

    resolve_selection(MENU_ITEMS[selection])
}

fn resolve_selection(label: &str) -> BlastResult<Option<Command>> {
    match label {
        "[APP] Refresh" => Ok(Some(Command::Refresh)),
        "[APP] Run Server" => Ok(Some(Command::Run)),
        "[APP] Watch Server" => Ok(Some(Command::Watch)),
        "[APP] Stop Server" => Ok(Some(Command::Stop)),
        "[APP] Toggle Dev/Prod" => Ok(Some(Command::ToggleEnv)),

        "[CODEGEN] Schema" => Ok(Some(Command::Schema)),
        "[CODEGEN] Structs" => Ok(Some(Command::Gen { cmd: Some(GenCmd::Structs) })),
        "[CODEGEN] Models" => Ok(Some(Command::Gen { cmd: Some(GenCmd::Models) })),
        "[CODEGEN] Gen Picker" => Ok(Some(Command::Gen { cmd: None })),
        "[CODEGEN] Gen Table (wizard)" => Ok(Some(Command::Gen { cmd: Some(GenCmd::Table) })),
        "[CODEGEN] Gen Migration (--custom)" => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Migration name (snake_case)")
                .interact_text()?;
            Ok(Some(Command::Gen {
                cmd: Some(GenCmd::Migration { custom: true, name: Some(name) }),
            }))
        }
        "[CODEGEN] Gen Frontend" => Ok(Some(Command::Gen { cmd: Some(GenCmd::Frontend) })),
        "[CODEGEN] Gen Governor Plugin" => {
            Ok(Some(Command::Gen { cmd: Some(GenCmd::GovernorPlugin) }))
        }
        "[CODEGEN] Gen Test Scaffolds" => Ok(Some(Command::Gen {
            cmd: Some(GenCmd::Test { flow: None, route: None }),
        })),

        "[DB] New Migration" => Ok(Some(Command::Migration)),
        "[DB] Migrate" => Ok(Some(Command::Migrate)),
        "[DB] Rollback" => Ok(Some(Command::Rollback)),
        "[DB] Seed" => Ok(Some(Command::Seed { file: None })),

        "[FUSES] Manage fuses" => Ok(Some(Command::Fuses { cmd: Some(FusesCmd::Interactive) })),
        "[FUSES] List fuses" => Ok(Some(Command::Fuses { cmd: Some(FusesCmd::List) })),

        "[LOG] Truncate Logs" => Ok(Some(Command::Log { cmd: LogCmd::Truncate { file: None } })),
        "[Arsenal] Scan & Write JSON" => Ok(Some(Command::Arsenal { cmd: None })),

        "[LINT] Governor Check" => Ok(Some(Command::Check { verbose: false })),

        "[Exit] Kill Session" => Ok(None),

        unknown => Err(crate::error::BlastError::Invalid(format!(
            "unknown menu selection: {}",
            unknown
        ))),
    }
}

pub fn run_interactive_loop(
    config: &mut Config,
    dep_manager: &mut DependencyManager,
) -> BlastResult<()> {
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
                drop(
                    std::process::Command::new("zellij")
                        .args(["kill-all-sessions", "-y"])
                        .spawn(),
                );
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
