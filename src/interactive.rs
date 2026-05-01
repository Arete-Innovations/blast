use std::{env, io::Write};

use crate::{
    commands::{ArsenalCmd, Command, FusesCmd, GenCmd, LogCmd},
    configs::Config,
    dependencies::DependencyManager,
    error::{BlastError, BlastResult},
    logger,
    wizards::widgets::{list_select, text_input},
};

// MENU_ITEMS is a hand-picked subset of `Command` — only the operations a user
// actually drives interactively from inside the dashboard. CI-scope commands
// (`build`, `package`), recursive ones (`dashboard` — the menu IS a dashboard
// pane), and codegen sub-passes (`schema`, `structs`, `models`,
// `governor-plugin` — all subsumed by `gen all`) stay reachable as
// `blast <subcommand>` from the shell. Same for `new` / `init` (project must
// not already exist). When adding a new menu entry, also add a resolve arm
// below.
const MENU_ITEMS: &[&str] = &[
    // ── app lifecycle ─────────────────────────────────────────────────────────
    "[APP] Watch (cargo leptos watch — BE+WASM HMR)",
    "[APP] Run Server (cargo leptos serve)",
    "[APP] Run Server (cargo leptos serve --release)",
    "[APP] Stop Server",
    "[APP] Refresh",
    "[APP] Toggle Dev/Prod",
    // ── codegen ───────────────────────────────────────────────────────────────
    "[CODEGEN] Gen All (full pipeline)",
    "[BUILD] Release build (cargo leptos build --release --precompress)",
    "[E2E] Run end-to-end tests (cargo leptos end-to-end)",
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
    // ── arsenal ───────────────────────────────────────────────────────────────
    "[ARSENAL] Scan & Write JSON",
    "[ARSENAL] Serve MCP (stdio)",
    // ── exit ──────────────────────────────────────────────────────────────────
    "[Exit] Kill Session",
];

enum SelectionOutcome {
    Resolved(Command),
    Quit,
    Cancelled,
}

pub fn pick_command(config: &Config) -> BlastResult<Option<Command>> {
    let prompt = format!("[{}]->[{}] — pick a command", config.environment.to_uppercase(), config.project_name);

    loop {
        let chosen = list_select::pick(&prompt, MENU_ITEMS)?;
        let label = match chosen {
            Some(i) => MENU_ITEMS[i],
            None => return Ok(None),
        };

        match resolve_selection(label)? {
            SelectionOutcome::Resolved(cmd) => return Ok(Some(cmd)),
            SelectionOutcome::Quit => return Ok(None),
            SelectionOutcome::Cancelled => continue,
        }
    }
}

fn resolve_selection(label: &str) -> BlastResult<SelectionOutcome> {
    let cmd = match label {
        // ── app lifecycle ─────────────────────────────────────────────────────
        "[APP] Refresh" => Command::Refresh,
        "[APP] Run Server (cargo leptos serve)" => Command::Run,
        "[APP] Run Server (cargo leptos serve --release)" => Command::RunProd,
        "[APP] Watch (cargo leptos watch — BE+WASM HMR)" => Command::Watch,
        "[APP] Stop Server" => Command::Stop,
        "[APP] Toggle Dev/Prod" => Command::ToggleEnv,

        // ── codegen ───────────────────────────────────────────────────────────
        "[CODEGEN] Gen All (full pipeline)" => Command::Gen { cmd: Some(GenCmd::All) },
        "[BUILD] Release build (cargo leptos build --release --precompress)" => Command::Build,
        "[E2E] Run end-to-end tests (cargo leptos end-to-end)" => Command::E2e,

        // ── database ──────────────────────────────────────────────────────────
        "[DB] New Migration" => Command::Migration,
        "[DB] Migrate" => Command::Migrate,
        "[DB] Rollback" => Command::Rollback,
        "[DB] Seed" => Command::Seed { file: None },

        // ── fuses ─────────────────────────────────────────────────────────────
        "[FUSES] Manage fuses (TUI)" => Command::Fuses { cmd: Some(FusesCmd::Interactive) },
        "[FUSES] List fuses" => Command::Fuses { cmd: Some(FusesCmd::List) },
        "[FUSES] Toggle fuse" => match text_input::ask("Fuse name", None)? {
            Some(name) if !name.trim().is_empty() => Command::Fuses {
                cmd: Some(FusesCmd::Toggle { name: name.trim().to_string() }),
            },
            _empty_or_cancel => return Ok(SelectionOutcome::Cancelled),
        },
        "[FUSES] Run fuse now" => match text_input::ask("Fuse name", None)? {
            Some(name) if !name.trim().is_empty() => Command::Fuses {
                cmd: Some(FusesCmd::Run { name: name.trim().to_string() }),
            },
            _empty_or_cancel => return Ok(SelectionOutcome::Cancelled),
        },
        "[FUSES] Fuse logs" => match text_input::ask("Fuse name", None)? {
            Some(name) if !name.trim().is_empty() => Command::Fuses {
                cmd: Some(FusesCmd::Logs { name: name.trim().to_string() }),
            },
            _empty_or_cancel => return Ok(SelectionOutcome::Cancelled),
        },
        "[FUSES] Live fuses table" => Command::Fuses { cmd: Some(FusesCmd::LiveTable) },

        // ── logs ──────────────────────────────────────────────────────────────
        "[LOG] View logs" => match text_input::ask("Log level (error/warn/info/debug)", Some("info"))? {
            Some(level) if !level.trim().is_empty() => Command::Log {
                cmd: LogCmd::View { level: level.trim().to_string() },
            },
            _empty_or_cancel => return Ok(SelectionOutcome::Cancelled),
        },
        "[LOG] Truncate Logs" => Command::Log { cmd: LogCmd::Truncate { file: None } },

        // ── arsenal ───────────────────────────────────────────────────────────
        "[ARSENAL] Scan & Write JSON" => Command::Arsenal { cmd: None },
        "[ARSENAL] Serve MCP (stdio)" => Command::Arsenal { cmd: Some(ArsenalCmd::Serve) },

        // ── exit ──────────────────────────────────────────────────────────────
        "[Exit] Kill Session" => return Ok(SelectionOutcome::Quit),

        unknown => return Err(BlastError::Invalid(format!("unknown menu selection: {}", unknown))),
    };

    Ok(SelectionOutcome::Resolved(cmd))
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
                let be_was_up = crate::daemon::stop_server(config)?;
                if be_was_up {
                    logger::success("BE daemon stopped")?;
                }
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

    fn unwrap_resolved(label: &str) -> Command {
        match resolve_selection(label).expect("resolve_selection ok") {
            SelectionOutcome::Resolved(c) => c,
            SelectionOutcome::Quit => panic!("expected Resolved, got Quit for {}", label),
            SelectionOutcome::Cancelled => panic!("expected Resolved, got Cancelled for {}", label),
        }
    }

    fn assert_quit(label: &str) {
        match resolve_selection(label).expect("resolve_selection ok") {
            SelectionOutcome::Quit => {}
            other_outcome => panic!(
                "expected Quit for {}, got {}",
                label,
                match other_outcome {
                    SelectionOutcome::Resolved(_) => "Resolved",
                    SelectionOutcome::Cancelled => "Cancelled",
                    SelectionOutcome::Quit => "Quit",
                }
            ),
        }
    }

    /// Items that do NOT require interactive prompting.
    /// Each must resolve to Resolved(_) or Quit without hitting Cancelled.
    #[test]
    fn non_interactive_menu_items_all_resolve() {
        let non_interactive: &[&str] = &[
            "[APP] Refresh",
            "[APP] Run Server (cargo leptos serve)",
            "[APP] Run Server (cargo leptos serve --release)",
            "[APP] Watch (cargo leptos watch — BE+WASM HMR)",
            "[APP] Stop Server",
            "[APP] Toggle Dev/Prod",
            "[CODEGEN] Gen All (full pipeline)",
            "[BUILD] Release build (cargo leptos build --release --precompress)",
            "[E2E] Run end-to-end tests (cargo leptos end-to-end)",
            "[DB] New Migration",
            "[DB] Migrate",
            "[DB] Rollback",
            "[DB] Seed",
            "[FUSES] Manage fuses (TUI)",
            "[FUSES] List fuses",
            "[FUSES] Live fuses table",
            "[LOG] Truncate Logs",
            "[ARSENAL] Scan & Write JSON",
            "[ARSENAL] Serve MCP (stdio)",
            "[Exit] Kill Session",
        ];

        // Every label in MENU_ITEMS must be listed in either non_interactive or
        // the interactive set below (i.e. there are no unhandled labels).
        let interactive_labels: &[&str] = &["[FUSES] Toggle fuse", "[FUSES] Run fuse now", "[FUSES] Fuse logs", "[LOG] View logs"];

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
        assert!(matches!(unwrap_resolved("[APP] Run Server (cargo leptos serve)"), Command::Run));
        assert!(matches!(unwrap_resolved("[APP] Run Server (cargo leptos serve --release)"), Command::RunProd));
        assert!(matches!(unwrap_resolved("[CODEGEN] Gen All (full pipeline)"), Command::Gen { cmd: Some(GenCmd::All) }));
        assert!(matches!(unwrap_resolved("[BUILD] Release build (cargo leptos build --release --precompress)"), Command::Build));
        assert!(matches!(unwrap_resolved("[E2E] Run end-to-end tests (cargo leptos end-to-end)"), Command::E2e));
        assert!(matches!(unwrap_resolved("[FUSES] Manage fuses (TUI)"), Command::Fuses { cmd: Some(FusesCmd::Interactive) }));
        assert!(matches!(unwrap_resolved("[FUSES] Live fuses table"), Command::Fuses { cmd: Some(FusesCmd::LiveTable) }));
        assert!(matches!(unwrap_resolved("[LOG] Truncate Logs"), Command::Log { cmd: LogCmd::Truncate { file: None } }));
        assert!(matches!(unwrap_resolved("[ARSENAL] Serve MCP (stdio)"), Command::Arsenal { cmd: Some(ArsenalCmd::Serve) }));
        assert_quit("[Exit] Kill Session");
    }
}
