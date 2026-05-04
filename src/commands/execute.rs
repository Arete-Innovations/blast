use clap::CommandFactory;

use crate::{
    commands::cli::{ArsenalCmd, Cli, Command, FusesCmd, GenCmd, LogCmd},
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
        Command::Gen { cmd: sub } => dispatch_gen(sub, config, dep_manager),
        Command::Log { cmd: sub } => dispatch_log(sub, config),
        Command::Arsenal { cmd: sub } => dispatch_arsenal(sub, config),

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
                post_seed: Some(std::sync::Arc::new(move |root, sink, progress| crate::commands::scaffold_post_seed::run(root, no_warmup, sink, progress))),
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
                post_seed: Some(std::sync::Arc::new(move |root, sink, progress| crate::commands::scaffold_post_seed::run(root, no_warmup, sink, progress))),
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
            // Reload config so subsequent commands see the freshly-scaffolded
            // project. Best-effort; the freshly-scaffolded project may live in
            // a different cwd than where blast was invoked from.
            match config.reload_if_modified() {
                Ok(_unit) => {} // allow: reload_if_modified returns () on success; nothing to bind
                Err(reload_err) => {
                    sink.warn(format!("config reload after init failed (non-fatal): {}", reload_err));
                }
            }
            // dep_manager is intentionally unused in this branch — init no
            // longer runs the legacy codegen pipeline that needed it.
            Ok(())
        }

        Command::Cli { menu } => crate::interactive::run_interactive_loop(config, dep_manager, menu),

        Command::Migration { name } => {
            match name {
                Some(migration_name) => {
                    let migration_dir = crate::database::write_migration(&migration_name, "", "")?;
                    let up_path = migration_dir.join("up.sql");
                    let down_path = migration_dir.join("down.sql");
                    logger::success(&format!("Migration skeleton written: {} ({})", up_path.display(), down_path.display(),))?;
                    logger::info("Edit up.sql / down.sql, then run `blast migrate` (and `blast gen schema && blast gen all` if you want codegen).")?;
                    return Ok(());
                }
                None => {} // allow: ERROR:3 forbids `if let Some`; fall through to interactive wizard below
            }

            let project_root = config.project_dir.clone();
            let outcome = crate::wizards::new_table::run_picker(&project_root)?;
            if outcome.cancelled {
                logger::info("New-table wizard cancelled — nothing written.")?;
                return Ok(());
            }
            logger::success(&format!("Migration written: {} ({})", outcome.up_sql_path.display(), outcome.down_sql_path.display(),))?;
            logger::success(&format!("Resource state written: {}", outcome.ron_path.display()))?;

            logger::info("→ blast migrate")?;
            if !crate::database::migrate() {
                logger::warning("Migrations reported issues; aborting codegen pipeline. Re-run `blast migrate && blast gen all` manually after resolving.")?;
                return Ok(());
            }

            logger::info("→ blast gen schema")?;
            run_schema(config)?;

            logger::info("→ blast gen all")?;
            run_gen_all(config)?;

            logger::success(&format!("Table '{}' is wired end-to-end.", outcome.table_name))?;
            Ok(())
        }

        Command::Sync { dev } => {
            let mut sink = crate::io::cli_sink(logger::is_verbose(), None);
            let mut progress = crate::io::cli_progress(None);
            crate::project::sync::run_sync(dev, &mut sink, &mut progress)?;
            Ok(())
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

        Command::Refresh => run_refresh(config),

        Command::Run => run_dev_server(config),
        Command::RunProd => run_prod_server(config),

        Command::Dashboard => {
            dep_manager.ensure_installed(&["zellij"])?;
            crate::dashboard::launch_dashboard(config)?;
            Ok(())
        }

        Command::ToggleEnv => {
            config.toggle_environment()?;
            logger::info("Run `blast scss`, `blast css`, or `blast js` to rebuild assets with new settings")?;
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

fn dispatch_gen(sub: Option<GenCmd>, config: &mut Config, _dep_manager: &mut DependencyManager) -> BlastResult<()> {
    let Some(target) = sub else {
        return print_gen_help();
    };
    match target {
        GenCmd::Structs => {
            let mut sink = crate::io::cli_sink(false, None);
            let mut progress = crate::io::cli_progress(None);
            let project_root = config.project_dir.clone();
            let report = crate::codegen::structs::run(&project_root, &mut sink, &mut progress)?;
            for p in &report.written {
                logger::success(&format!("wrote {}", p.display()))?;
            }
            logger::info(&format!("structs: {} written, {} skipped", report.written.len(), report.skipped.len(),))?;
            Ok(())
        }
        GenCmd::Enums { resource: _ } => run_gen_enums(config),
        GenCmd::Models => {
            if !crate::models::generate(config) {
                logger::warning("Some model generation issues occurred")?;
            }
            Ok(())
        }
        GenCmd::Validators { resource } => run_gen_validators(config, resource),
        GenCmd::All => run_gen_all(config),
    }
}

fn run_gen_validators(config: &Config, resource: Option<String>) -> BlastResult<()> {
    let mut sink = crate::io::cli_sink(logger::is_verbose(), None);
    let mut progress = crate::io::cli_progress(None);
    let report = match resource {
        Some(name) => crate::codegen::validators::run_for_resource(&config.project_dir, &name, &mut sink, &mut progress)?,
        None => crate::codegen::validators::run(&config.project_dir, &mut sink, &mut progress)?,
    };
    logger::info(&format!("validators: {} file(s) written, {} skipped", report.written.len(), report.skipped.len()))?;
    Ok(())
}

fn run_gen_enums(config: &Config) -> BlastResult<()> {
    let mut sink = crate::io::cli_sink(logger::is_verbose(), None);
    let mut progress = crate::io::cli_progress(None);
    let report = crate::codegen::enums::run(&config.project_dir, &mut sink, &mut progress)?;
    for p in &report.written {
        logger::success(&format!("wrote {}", p.display()))?;
    }
    logger::info(&format!("enums: {} file(s) written, {} skipped", report.written.len(), report.skipped.len()))?;
    Ok(())
}

fn run_gen_all(config: &mut Config) -> BlastResult<()> {
    let verbose = logger::is_verbose();
    let mut sink = crate::io::cli_sink(verbose, None);
    let mut progress = crate::io::cli_progress(None);
    let args = crate::commands::gen_all::Args { project_root: config.project_dir.clone() };
    let outcome = crate::commands::gen_all::run(args, config, &mut sink, &mut progress)?;
    logger::info(&format!(
        "gen all complete: {} steps, {} files written, {} files skipped",
        outcome.steps_run, outcome.files_written, outcome.files_skipped
    ))?;
    Ok(())
}

fn print_gen_help() -> BlastResult<()> {
    let mut cmd = Cli::command();
    match cmd.find_subcommand_mut("gen") {
        Some(gen) => {
            gen.print_help()?;
            println!();
        }
        None => {} // allow: clap always registers the gen subcommand; defensive None arm
    }
    Ok(())
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

fn dispatch_arsenal(sub: Option<ArsenalCmd>, config: &Config) -> BlastResult<()> {
    match sub {
        None => {
            logger::info("Scanning source for arsenal report...")?;
            let report = crate::arsenal::scanner::scan(&config.project_dir)?;
            crate::arsenal::report::write_report(&report, &config.project_dir)?;
            let total_entries: usize = report.layers.values().map(|v| v.len()).sum();
            logger::success(&format!(
                "arsenal: {} entries across {} layers, {} routes -> target/arsenal.json",
                total_entries,
                report.layers.len(),
                report.routes.len(),
            ))?;
            Ok(())
        }
        Some(ArsenalCmd::Serve) => {
            let report = crate::arsenal::scanner::scan(&config.project_dir)?;
            crate::arsenal::mcp::serve(report)
        }
    }
}

fn run_schema(config: &Config) -> BlastResult<()> {
    if !crate::database::generate_schema() {
        logger::warning("Some schema generation issues occurred")?;
    }
    let schema_path = config.project_dir.join("src/database/schema.rs");
    match crate::schema_parser::parse_schema(&schema_path) {
        Ok(tables) => {
            let col_count: usize = tables.iter().map(|t| t.columns.len()).sum();
            let nullable_count: usize = tables.iter().flat_map(|t| t.columns.iter()).filter(|c| c.nullable).count();
            logger::info(&format!(
                "schema: {} tables, {} columns ({} nullable) — pk fields: {}",
                tables.len(),
                col_count,
                nullable_count,
                tables.iter().map(|t| format!("{}({})", t.name, t.primary_key.join(","))).collect::<Vec<_>>().join(", "),
            ))?;
            for t in &tables {
                logger::debug(&format!(
                    "  {} cols: {}",
                    t.name,
                    t.columns
                        .iter()
                        .map(|c| format!("{}:{}{}", c.name, c.diesel_type, if c.nullable { "?" } else { "" }))
                        .collect::<Vec<_>>()
                        .join(", "),
                ))?;
            }
        }
        Err(e) => {
            logger::warning(&format!("schema parse warning: {}", e))?;
        }
    }
    Ok(())
}

fn run_refresh(config: &mut Config) -> BlastResult<()> {
    let mut progress = logger::create_progress(None);

    progress.set_message("Rolling back migrations...");
    let rollback_ok = crate::database::rollback_all();

    progress.set_message("Running migrations...");
    let migrations_ok = crate::database::migrate();

    progress.set_message("Seeding database...");
    let seed_ok = crate::database::seed();

    progress.set_message("Generating schema...");
    let schema_ok = crate::database::generate_schema();

    progress.set_message("Generating structs...");
    let mut sink = crate::io::cli_sink(false, None);
    let mut structs_progress = crate::io::cli_progress(None);
    let structs_ok = crate::codegen::structs::run(&config.project_dir, &mut sink, &mut structs_progress).is_ok();

    progress.set_message("Generating models...");
    let models_ok = crate::models::generate(config);

    if rollback_ok && migrations_ok && seed_ok && schema_ok && structs_ok && models_ok {
        progress.success("App refresh complete!");
    } else {
        progress.error("App refresh completed with some issues");
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
