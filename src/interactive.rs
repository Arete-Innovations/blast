use std::{env, io::Write};

use crate::{
    commands::{ArsenalCmd, Command, FusesCmd, GenCmd, LogCmd, MenuKind},
    configs::Config,
    dependencies::DependencyManager,
    error::{BlastError, BlastResult},
    logger,
    wizards::widgets::{list_select, text_input},
};

const APP_MENU: &[&str] = &[
    // ── app lifecycle ─────────────────────────────────────────────────────────
    "[APP] Watch dev (cargo leptos watch)",
    "[APP] Watch prod (cargo leptos watch --release --precompress)",
    // ── codegen ───────────────────────────────────────────────────────────────
    "[CODEGEN] Gen All (full pipeline)",
    "[BUILD] Release build (cargo leptos build --release --precompress)",
    "[E2E] Run end-to-end tests (cargo leptos end-to-end)",
    // ── database ──────────────────────────────────────────────────────────────
    "[DB] New Migration",
    "[DB] Migrate",
    "[DB] Rollback",
    "[DB] Seed",
    // ── logs ──────────────────────────────────────────────────────────────────
    "[LOG] View logs",
    "[LOG] Truncate Logs",
    // ── arsenal ───────────────────────────────────────────────────────────────
    "[ARSENAL] Scan & Write JSON",
    "[ARSENAL] Serve MCP (stdio)",
    // ── exit ──────────────────────────────────────────────────────────────────
    "[Exit] Kill Session",
];

const FUSES_MENU: &[&str] = &[
    "[FUSES] Manage fuses (TUI)",
    "[FUSES] List fuses",
    "[FUSES] Toggle fuse",
    "[FUSES] Run fuse now",
    "[FUSES] Fuse logs",
    "[FUSES] Live fuses table",
    "[Exit] Close menu",
];

enum SelectionOutcome {
    Resolved(Command),
    Quit,
    Cancelled,
}

fn menu_items(kind: MenuKind) -> &'static [&'static str] {
    match kind {
        MenuKind::App => APP_MENU,
        MenuKind::Fuses => FUSES_MENU,
    }
}

fn menu_title(kind: MenuKind) -> &'static str {
    match kind {
        MenuKind::App => "app",
        MenuKind::Fuses => "fuses",
    }
}

pub fn pick_command(config: &Config, kind: MenuKind) -> BlastResult<Option<Command>> {
    let prompt = format!("[{}]->[{}]->[{}] — pick a command", config.environment.to_uppercase(), config.project_name, menu_title(kind));
    let items = menu_items(kind);

    loop {
        let chosen = list_select::pick(&prompt, items)?;
        let label = match chosen {
            Some(i) => items[i],
            None => return Ok(None),
        };

        match resolve_selection(label, kind)? {
            SelectionOutcome::Resolved(cmd) => return Ok(Some(cmd)),
            SelectionOutcome::Quit => return Ok(None),
            SelectionOutcome::Cancelled => continue,
        }
    }
}

fn resolve_selection(label: &str, kind: MenuKind) -> BlastResult<SelectionOutcome> {
    match kind {
        MenuKind::App => resolve_app(label),
        MenuKind::Fuses => resolve_fuses(label),
    }
}

fn resolve_app(label: &str) -> BlastResult<SelectionOutcome> {
    let cmd = match label {
        "[APP] Watch dev (cargo leptos watch)" => Command::Watch,
        "[APP] Watch prod (cargo leptos watch --release --precompress)" => Command::WatchProd,

        "[CODEGEN] Gen All (full pipeline)" => Command::Gen { cmd: Some(GenCmd::All) },
        "[BUILD] Release build (cargo leptos build --release --precompress)" => Command::Build,
        "[E2E] Run end-to-end tests (cargo leptos end-to-end)" => Command::E2e,

        "[DB] New Migration" => Command::Migration { name: None },
        "[DB] Migrate" => Command::Migrate,
        "[DB] Rollback" => Command::Rollback,
        "[DB] Seed" => Command::Seed { file: None },

        "[LOG] View logs" => match text_input::ask("Log level (error/warn/info/debug)", Some("info"))? {
            Some(level) if !level.trim().is_empty() => Command::Log {
                cmd: LogCmd::View { level: level.trim().to_string() },
            },
            _empty_or_cancel => return Ok(SelectionOutcome::Cancelled),
        },
        "[LOG] Truncate Logs" => Command::Log { cmd: LogCmd::Truncate { file: None } },

        "[ARSENAL] Scan & Write JSON" => Command::Arsenal { cmd: None },
        "[ARSENAL] Serve MCP (stdio)" => Command::Arsenal { cmd: Some(ArsenalCmd::Serve) },

        "[Exit] Kill Session" => return Ok(SelectionOutcome::Quit),

        unknown => return Err(BlastError::Invalid(format!("unknown app-menu selection: {}", unknown))),
    };

    Ok(SelectionOutcome::Resolved(cmd))
}

fn resolve_fuses(label: &str) -> BlastResult<SelectionOutcome> {
    let cmd = match label {
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

        "[Exit] Close menu" => return Ok(SelectionOutcome::Quit),

        unknown => return Err(BlastError::Invalid(format!("unknown fuses-menu selection: {}", unknown))),
    };

    Ok(SelectionOutcome::Resolved(cmd))
}

pub fn run_interactive_loop(config: &mut Config, dep_manager: &mut DependencyManager, kind: MenuKind) -> BlastResult<()> {
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

        let picked = pick_command(config, kind)?;

        let cmd = match picked {
            Some(c) => c,
            None => {
                if matches!(kind, MenuKind::App) {
                    let be_was_up = crate::daemon::stop_server(config)?;
                    if be_was_up {
                        logger::success("BE daemon stopped")?;
                    }
                    drop(std::process::Command::new("zellij").args(["kill-session"]).spawn());
                    drop(std::process::Command::new("zellij").args(["kill-all-sessions", "-y"]).spawn());
                }
                break;
            }
        };

        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush()?;

        match crate::commands::execute(cmd, config, dep_manager) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{ArsenalCmd, FusesCmd, GenCmd, LogCmd};

    fn unwrap_resolved(label: &str, kind: MenuKind) -> Command {
        match resolve_selection(label, kind).expect("resolve_selection ok") {
            SelectionOutcome::Resolved(c) => c,
            SelectionOutcome::Quit => panic!("expected Resolved, got Quit for {}", label),
            SelectionOutcome::Cancelled => panic!("expected Resolved, got Cancelled for {}", label),
        }
    }

    fn assert_quit(label: &str, kind: MenuKind) {
        match resolve_selection(label, kind).expect("resolve_selection ok") {
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

    #[test]
    fn app_menu_non_interactive_items_resolve() {
        let non_interactive: &[&str] = &[
            "[APP] Watch dev (cargo leptos watch)",
            "[APP] Watch prod (cargo leptos watch --release --precompress)",
            "[CODEGEN] Gen All (full pipeline)",
            "[BUILD] Release build (cargo leptos build --release --precompress)",
            "[E2E] Run end-to-end tests (cargo leptos end-to-end)",
            "[DB] New Migration",
            "[DB] Migrate",
            "[DB] Rollback",
            "[DB] Seed",
            "[LOG] Truncate Logs",
            "[ARSENAL] Scan & Write JSON",
            "[ARSENAL] Serve MCP (stdio)",
            "[Exit] Kill Session",
        ];

        let interactive_labels: &[&str] = &["[LOG] View logs"];

        let all_handled: Vec<&str> = non_interactive.iter().chain(interactive_labels.iter()).copied().collect();

        for label in APP_MENU {
            assert!(all_handled.contains(label), "APP_MENU label {:?} is not tracked in the parity test — add it", label);
        }

        for label in all_handled.iter() {
            assert!(APP_MENU.contains(label), "parity test lists {:?} but it is not in APP_MENU", label);
        }
    }

    #[test]
    fn fuses_menu_non_interactive_items_resolve() {
        let non_interactive: &[&str] = &[
            "[FUSES] Manage fuses (TUI)",
            "[FUSES] List fuses",
            "[FUSES] Live fuses table",
            "[Exit] Close menu",
        ];

        let interactive_labels: &[&str] = &["[FUSES] Toggle fuse", "[FUSES] Run fuse now", "[FUSES] Fuse logs"];

        let all_handled: Vec<&str> = non_interactive.iter().chain(interactive_labels.iter()).copied().collect();

        for label in FUSES_MENU {
            assert!(all_handled.contains(label), "FUSES_MENU label {:?} is not tracked in the parity test — add it", label);
        }

        for label in all_handled.iter() {
            assert!(FUSES_MENU.contains(label), "parity test lists {:?} but it is not in FUSES_MENU", label);
        }
    }

    #[test]
    fn app_menu_routing_spot_check() {
        assert!(matches!(unwrap_resolved("[APP] Watch dev (cargo leptos watch)", MenuKind::App), Command::Watch));
        assert!(matches!(unwrap_resolved("[APP] Watch prod (cargo leptos watch --release --precompress)", MenuKind::App), Command::WatchProd));
        assert!(matches!(unwrap_resolved("[CODEGEN] Gen All (full pipeline)", MenuKind::App), Command::Gen { cmd: Some(GenCmd::All) }));
        assert!(matches!(unwrap_resolved("[BUILD] Release build (cargo leptos build --release --precompress)", MenuKind::App), Command::Build));
        assert!(matches!(unwrap_resolved("[E2E] Run end-to-end tests (cargo leptos end-to-end)", MenuKind::App), Command::E2e));
        assert!(matches!(unwrap_resolved("[LOG] Truncate Logs", MenuKind::App), Command::Log { cmd: LogCmd::Truncate { file: None } }));
        assert!(matches!(unwrap_resolved("[ARSENAL] Serve MCP (stdio)", MenuKind::App), Command::Arsenal { cmd: Some(ArsenalCmd::Serve) }));
        assert_quit("[Exit] Kill Session", MenuKind::App);
    }

    #[test]
    fn fuses_menu_routing_spot_check() {
        assert!(matches!(unwrap_resolved("[FUSES] Manage fuses (TUI)", MenuKind::Fuses), Command::Fuses { cmd: Some(FusesCmd::Interactive) }));
        assert!(matches!(unwrap_resolved("[FUSES] List fuses", MenuKind::Fuses), Command::Fuses { cmd: Some(FusesCmd::List) }));
        assert!(matches!(unwrap_resolved("[FUSES] Live fuses table", MenuKind::Fuses), Command::Fuses { cmd: Some(FusesCmd::LiveTable) }));
        assert_quit("[Exit] Close menu", MenuKind::Fuses);
    }

    #[test]
    fn cross_menu_labels_rejected() {
        // App labels must NOT resolve under the Fuses menu, and vice versa.
        assert!(resolve_selection("[FUSES] List fuses", MenuKind::App).is_err());
        assert!(resolve_selection("[APP] Watch dev (cargo leptos watch)", MenuKind::Fuses).is_err());
    }
}
