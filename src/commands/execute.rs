use clap::CommandFactory;

use crate::{
    commands::cli::{Cli, Command, FusesCmd, LogCmd},
    configs::Config,
    dependencies::DependencyManager,
    error::{BlastError, BlastResult},
    io::traits::SinkExt,
    logger,
};

pub fn print_help() -> BlastResult<()> {
    let mut cmd = Cli::command();
    cmd.print_help()?;
    println!();
    Ok(())
}

pub fn execute(cmd: Command, config: &mut Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    if !is_config_independent(&cmd) {
        if let Err(e) = config.reload_if_modified() {
            logger::warning(&format!("Failed to reload config: {}", e))?;
        }
    }

    match cmd {
        Command::Fuses { cmd: sub } => dispatch_fuses(sub, config),
        Command::Log { cmd: sub } => dispatch_log(sub, config),

        Command::Stop => {
            let be_stopped = crate::daemon::stop_server(config)?;
            match be_stopped {
                true => logger::success("BE stopped")?,
                false => logger::success("Nothing was running")?,
            }
            Ok(())
        }

        Command::New {
            name,
            dev,
            db_url,
            force,
            no_test_db,
            no_warmup,
        } => {
            let mut sink = crate::io::cli_sink(logger::is_verbose(), None);
            let mut progress = crate::io::cli_progress(None);
            let opts = crate::project::scaffold::NewOptions {
                db_url,
                force,
                no_test_db,
                no_warmup,
                dev,
            };
            crate::project::scaffold::create_new_project_with_opts(&name, opts, &mut sink, &mut progress)?;
            Ok(())
        }

        Command::Init {
            name,
            db_url,
            force,
            no_test_db,
            no_warmup,
        } => {
            let mut sink = crate::io::cli_sink(logger::is_verbose(), None);
            let mut progress = crate::io::cli_progress(None);
            let opts = crate::project::scaffold::NewOptions {
                db_url,
                force,
                no_test_db,
                no_warmup,
                dev: false,
            };
            match name {
                Some(n) => {
                    crate::project::scaffold::create_new_project_with_opts(&n, opts, &mut sink, &mut progress)?;
                }
                None => {
                    let cwd = std::env::current_dir()?;
                    let project_name = cwd
                        .file_name()
                        .and_then(|n| n.to_str())
                        .ok_or_else(|| BlastError::Invalid("could not derive project name from current dir".to_string()))?
                        .to_string();
                    crate::project::scaffold::init_in_place_with_opts(&project_name, cwd, opts, &mut sink, &mut progress)?;
                }
            }
            match config.reload_if_modified() {
                Ok(_unit) => {}
                Err(reload_err) => {
                    sink.warn(format!("config reload after init failed (non-fatal): {}", reload_err));
                }
            }
            Ok(())
        }

        Command::Cli { menu } => crate::interactive::run_interactive_loop(config, dep_manager, menu),

        Command::Migration { name } => {
            let migration_dir = crate::database::write_migration(&name, "", "")?;
            let up_path = migration_dir.join("up.sql");
            let down_path = migration_dir.join("down.sql");
            logger::success(&format!("Migration skeleton written: {} ({})", up_path.display(), down_path.display()))?;
            logger::info("Edit up.sql / down.sql, then run `blast migrate && blast schema`.")?;
            Ok(())
        }

        Command::Sync { dev, dry_run } => {
            let mut sink = crate::io::cli_sink(logger::is_verbose(), None);
            let mut progress = crate::io::cli_progress(None);
            crate::project::sync::run_sync(dev, dry_run, &mut sink, &mut progress)
        }

        Command::Migrate => {
            if !crate::database::migrate() {
                logger::warning("Some migration issues occurred")?;
            }
            Ok(())
        }

        Command::Rollback => {
            if !crate::database::rollback_all() {
                logger::warning("Some rollback issues occurred")?;
            }
            Ok(())
        }

        Command::Seed { file } => {
            let success = match file {
                Some(path) => crate::database::seed_specific_file(&path),
                None => crate::database::seed(),
            };
            if !success {
                logger::warning("Some seeding issues occurred")?;
            }
            Ok(())
        }

        Command::Schema => run_schema(config),

        Command::Build => crate::build::run_build(config),
        Command::Package => crate::build::run_package(config),

        Command::Run => run_dev_server(config),
        Command::RunProd => run_prod_server(config),

        Command::Dashboard => {
            dep_manager.ensure_installed(&["zellij"])?;
            crate::dashboard::launch_dashboard(config)?;
            Ok(())
        }

        Command::ToggleEnv => {
            config.toggle_environment()?;
            logger::info("Environment flipped — restart watchers/servers as needed")?;
            Ok(())
        }

        Command::Help => print_help(),

        Command::Watch => run_watch(config, dep_manager),
        Command::WatchProd => run_watch_prod(config, dep_manager),

        Command::E2e => run_e2e(config, dep_manager),
    }
}

fn is_config_independent(cmd: &Command) -> bool {
    matches!(cmd, Command::Help | Command::New { .. } | Command::Init { .. })
}

fn dispatch_fuses(sub: Option<FusesCmd>, config: &Config) -> BlastResult<()> {
    let resolved = match sub {
        Some(s) => s,
        None => FusesCmd::Interactive,
    };
    match resolved {
        FusesCmd::List => crate::fuses::list_fuses(config),
        FusesCmd::Toggle { name } => crate::fuses::toggle_fuse(config, &name),
        FusesCmd::Run { name } => {
            logger::info(&format!("Triggering immediate run of fuse '{}'...", name))?;
            crate::fuses::run_fuse(config, &name)
        }
        FusesCmd::Logs { name } => crate::fuses::logs_fuse(config, &name),
        FusesCmd::Interactive => {
            logger::info("Launching interactive fuses manager...")?;
            let mut sink = crate::io::cli_sink(false, None);
            let mut progress = crate::io::cli_progress(None);
            crate::fuses_tui::run_with_picker(config, &mut sink, &mut progress)?;
            Ok(())
        }
        FusesCmd::LiveTable => {
            logger::info("Launching live fuses table view...")?;
            crate::fuses_tui::display_fuses_table(config)
        }
    }
}

fn dispatch_log(sub: LogCmd, config: &Config) -> BlastResult<()> {
    match sub {
        LogCmd::Truncate { file } => {
            logger::info("Managing log files...")?;
            crate::logger::ensure_log_files_exist(config)?;
            crate::logger::truncate_specific_log(config, file)
        }
        LogCmd::View { level } => match crate::tui_viewer::run_tui_log_viewer(&level, config) {
            Ok(_v) => Ok(()),
            Err(e) => {
                logger::warning(&format!("TUI viewer failed ({}), falling back to simple viewer", e))?;
                crate::logger::view_logs_enhanced(&level, config)
            }
        },
    }
}

fn run_schema(_config: &Config) -> BlastResult<()> {
    if !crate::database::generate_schema() {
        logger::warning("Some schema generation issues occurred")?;
    }
    Ok(())
}

fn run_dev_server(config: &Config) -> BlastResult<()> {
    let pid = crate::daemon::start_server(config, crate::daemon::ServerMode::Dev)?;
    logger::success(&format!("Development server started with PID: {}", pid))?;
    Ok(())
}

fn run_prod_server(config: &Config) -> BlastResult<()> {
    let pid = crate::daemon::start_server(config, crate::daemon::ServerMode::Prod)?;
    logger::success(&format!("Production server started with PID: {}", pid))?;
    Ok(())
}

fn run_watch(config: &Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    dep_manager.ensure_installed(&["cargo-leptos"])?;
    let be_pid = crate::daemon::start_server(config, crate::daemon::ServerMode::Watch)?;
    logger::success(&format!("cargo leptos watch started — PID {} → storage/logs/server.log", be_pid))?;
    Ok(())
}

fn run_watch_prod(config: &Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    dep_manager.ensure_installed(&["cargo-leptos"])?;
    let be_pid = crate::daemon::start_server(config, crate::daemon::ServerMode::WatchProd)?;
    logger::success(&format!("cargo leptos watch --release started — PID {} → storage/logs/server.log", be_pid))?;
    Ok(())
}

fn run_e2e(config: &Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    dep_manager.ensure_installed(&["cargo-leptos"])?;
    let status = std::process::Command::new("cargo")
        .args(["leptos", "end-to-end"])
        .current_dir(&config.project_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err(crate::error::BlastError::Subprocess {
            cmd: "cargo leptos end-to-end".to_string(),
            detail: format!("exited with status {}", status),
        });
    }
    logger::success("end-to-end tests passed")?;
    Ok(())
}
