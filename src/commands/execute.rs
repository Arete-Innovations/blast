use crate::commands::cli::{ArsenalCmd, Cli, Command, FusesCmd, GenCmd, LogCmd};
use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::error::{BlastError, BlastResult};
use crate::logger;
use clap::CommandFactory;
use std::io::Write;

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
            logger::info("Stopping running server...")?;
            if let Err(e) = crate::dashboard::stop_server() {
                logger::error(&format!("Failed to stop server: {}", e))?;
                return Err(e);
            }
            logger::success("Server stopped successfully")?;
            Ok(())
        }

        Command::New { name, dev } => {
            crate::project::create_new_project(&name, dev);
            Ok(())
        }

        Command::Init => run_init(config, dep_manager),

        Command::Cli => crate::interactive::run_interactive_loop(config, dep_manager),

        Command::Migration => {
            let mut sink = crate::io::cli_sink(false, None);
            let mut progress = crate::io::cli_progress(None);
            crate::database::migration_wizard::run_with_picker(&mut sink, &mut progress)?;
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
                None => crate::database::seed(Some(0)),
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
            dep_manager.ensure_installed(&["zellij"], false)?;
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

        Command::Check { verbose } => run_check(config, verbose),
    }
}

fn is_config_independent(cmd: &Command) -> bool {
    matches!(cmd, Command::Help | Command::New { .. })
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
            let mut sink = crate::io::NullSink;
            let mut progress = crate::io::NullProgress;
            crate::fuses_tui::run_with_picker(config, &mut sink, &mut progress)?;
            Ok(())
        }
        FusesCmd::LiveTable => {
            logger::info("Launching live fuses table view...")?;
            crate::fuses_tui::display_fuses_table(config)
        }
    }
}

fn dispatch_gen(
    sub: Option<GenCmd>,
    config: &mut Config,
    dep_manager: &mut DependencyManager,
) -> BlastResult<()> {
    let Some(target) = sub else {
        let chosen = crate::gen_picker::pick_gen_target()?;
        return execute(chosen, config, dep_manager);
    };
    match target {
        GenCmd::Structs => {
            if !crate::structs::generate(config) {
                logger::warning("Some struct generation issues occurred")?;
            }
            Ok(())
        }
        GenCmd::Models => {
            if !crate::models::generate(config) {
                logger::warning("Some model generation issues occurred")?;
            }
            Ok(())
        }
        GenCmd::Table => {
            let mut sink = crate::io::cli_sink(false, None);
            let mut progress = crate::io::cli_progress(None);
            let project_root = config.project_dir.clone();
            match crate::gen_table::run_with_picker(&project_root, &mut sink, &mut progress) {
                Ok(_outcome) => Ok(()),
                Err(crate::error::BlastError::Invalid(msg)) if msg.contains("cancelled") => Ok(()),
                Err(e) => Err(e),
            }
        }
        GenCmd::Migration { custom, name } => {
            if !custom {
                return Err(BlastError::Invalid(
                    "blast gen migration requires --custom <name>".to_string(),
                ));
            }
            let resolved = name.ok_or_else(|| {
                BlastError::Invalid("blast gen migration --custom requires a name".to_string())
            })?;
            crate::gen_migration::run_custom(&resolved)
        }
        GenCmd::Frontend => {
            logger::info("Generating frontend artifacts from primer IR...")?;
            crate::codegen::run_frontend(&config.project_dir)?;
            logger::success("Frontend codegen complete")?;
            Ok(())
        }
        GenCmd::GovernorPlugin => {
            logger::info("Emitting governor Vite plugin shim...")?;
            let paths = crate::codegen::governor_plugin::run(&config.project_dir)?;
            for p in &paths {
                logger::success(&format!("emitted {}", p.display()))?;
            }
            Ok(())
        }
        GenCmd::FeScaffold => {
            logger::info("Seeding frontend scaffold (tokens.css, base.css, primevue.ts)...")?;
            let outcome = crate::codegen::frontend_scaffold::run(&config.project_dir)?;
            for p in &outcome.written {
                logger::success(&format!("seeded {}", p.display()))?;
            }
            for p in &outcome.skipped {
                logger::info(&format!("skipped {} (already present)", p.display()))?;
            }
            Ok(())
        }
        GenCmd::Resource { name } => run_gen_resource(config, name),
        GenCmd::Test { flow, route } => {
            let filter = resolve_test_filter(flow, route);
            logger::info("Scaffolding test files from primer IR...")?;
            let report = crate::codegen::test_scaffold::run(&config.project_dir, &filter)?;
            for path in &report.written {
                logger::success(&format!("wrote {}", path.display()))?;
            }
            logger::info(&format!(
                "test scaffold: {} written, {} skipped (already present)",
                report.written.len(),
                report.skipped.len(),
            ))?;
            Ok(())
        }
        GenCmd::All => run_gen_all(config),
        GenCmd::Resource { name } => run_gen_resource(config, name),
    }
}

fn run_gen_all(config: &mut Config) -> BlastResult<()> {
    let verbose = logger::is_verbose();
    let mut sink = crate::io::cli_sink(verbose, None);
    let mut progress = crate::io::cli_progress(None);
    let args = crate::commands::gen_all::Args {
        project_root: config.project_dir.clone(),
    };
    let outcome = crate::commands::gen_all::run(args, config, &mut sink, &mut progress)?;
    logger::info(&format!(
        "gen all complete: {} steps, {} files written, {} files skipped",
        outcome.steps_run, outcome.files_written, outcome.files_skipped
    ))?;
    Ok(())
}

fn run_gen_resource(config: &Config, name: Option<String>) -> BlastResult<()> {
    let project_root = config.project_dir.clone();
    let args = crate::wizards::gen_resource::pick_args_with_name(project_root, name)?;
    let mut sink = crate::io::cli_sink(logger::is_verbose(), None);
    let mut progress = crate::io::cli_progress(None);
    let outcome = crate::wizards::gen_resource::run(args, &mut sink, &mut progress)?;
    match outcome.action {
        crate::wizards::gen_resource::WriteAction::Created => {
            logger::success(&format!("created {}", outcome.state_file.display()))?;
        }
        crate::wizards::gen_resource::WriteAction::Updated => {
            logger::success(&format!("updated {}", outcome.state_file.display()))?;
        }
        crate::wizards::gen_resource::WriteAction::Cancelled => {
            logger::info("resource wizard cancelled, no file written")?;
        }
    }
    logger::info("run `blast gen all` to regenerate code from the new state")?;
    Ok(())
}

fn resolve_test_filter(
    flow: Option<String>,
    route: Option<String>,
) -> crate::codegen::test_scaffold::Filter {
    match flow {
        Some(name) => crate::codegen::test_scaffold::Filter::Flow(name),
        None => match route {
            Some(name) => crate::codegen::test_scaffold::Filter::Route(name),
            None => crate::codegen::test_scaffold::Filter::All,
        },
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

// blast init pipeline (per SPEC_BLAST_COMMANDS.md):
//   step 1/4: dependency check  — cargo, diesel_cli, node, zellij
//   step 2/4: database reset    — rollback all then migrate
//   step 3/4: seed data         — no-op if no seed file
//   step 4/4: codegen pipeline  — delegates to gen_all
fn run_init(config: &mut Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    logger::info("initializing project...")?;

    // Step 1/4: dependency check
    logger::info("init 1/4: checking dependencies")?;
    dep_manager.ensure_installed(&["cargo", "diesel", "node", "zellij"], false)?;

    // Step 2/4: database reset (rollback then migrate — idempotent)
    logger::info("init 2/4: resetting database (rollback + migrate)")?;
    if !crate::database::rollback_all() {
        logger::warning("rollback had issues — proceeding with migrate")?;
    }
    if !crate::database::migrate() {
        logger::warning("some migration issues occurred — check database configuration")?;
    }

    // Step 3/4: seed data
    logger::info("init 3/4: seeding database")?;
    if !crate::database::seed(None) {
        logger::warning("some seeding issues occurred — normal for new projects")?;
    }

    // Step 4/4: full codegen pipeline (schema → structs → models → flows → frontend → …)
    logger::info("init 4/4: running codegen pipeline")?;
    let verbose = logger::is_verbose();
    let mut sink = crate::io::cli_sink(verbose, None);
    let mut progress = crate::io::cli_progress(None);
    let args = crate::commands::gen_all::Args {
        project_root: config.project_dir.clone(),
    };
    let outcome = crate::commands::gen_all::run(args, config, &mut sink, &mut progress)?;

    logger::success(&format!(
        "init complete: {} codegen steps, {} files written, {} files skipped",
        outcome.steps_run, outcome.files_written, outcome.files_skipped
    ))?;
    logger::info("run 'blast run' to start the development server")?;
    logger::info("run 'blast dashboard' to launch the interactive dashboard")?;

    Ok(())
}

fn run_schema(config: &Config) -> BlastResult<()> {
    if !crate::database::generate_schema() {
        logger::warning("Some schema generation issues occurred")?;
    }
    let schema_path = config.project_dir.join("src/database/schema.rs");
    match crate::schema_parser::parse_schema(&schema_path) {
        Ok(tables) => {
            let col_count: usize = tables.iter().map(|t| t.columns.len()).sum();
            let nullable_count: usize = tables
                .iter()
                .flat_map(|t| t.columns.iter())
                .filter(|c| c.nullable)
                .count();
            logger::info(&format!(
                "schema: {} tables, {} columns ({} nullable) — pk fields: {}",
                tables.len(),
                col_count,
                nullable_count,
                tables
                    .iter()
                    .map(|t| format!("{}({})", t.name, t.primary_key.join(",")))
                    .collect::<Vec<_>>()
                    .join(", "),
            ))?;
            for t in &tables {
                logger::debug(&format!(
                    "  {} cols: {}",
                    t.name,
                    t.columns
                        .iter()
                        .map(|c| format!(
                            "{}:{}{}",
                            c.name,
                            c.diesel_type,
                            if c.nullable { "?" } else { "" }
                        ))
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
    let seed_ok = crate::database::seed(Some(0));

    progress.set_message("Generating schema...");
    let schema_ok = crate::database::generate_schema();

    progress.set_message("Generating structs...");
    let structs_ok = crate::structs::generate(config);

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
    match crate::dashboard::start_server(config, true) {
        Ok(pid) => {
            logger::success(&format!("Development server started with PID: {}", pid))?;
        }
        Err(_started) => {
            let cmd = format!("cargo run --bin {}", &config.project_name);
            std::process::Command::new("script")
                .args(["-q", "-c", &cmd, "storage/logs/server.log"])
                .spawn()?;
            logger::success("Development server started with cargo run")?;
        }
    }
    Ok(())
}

fn run_prod_server(config: &Config) -> BlastResult<()> {
    match crate::dashboard::start_server(config, false) {
        Ok(pid) => {
            logger::success(&format!("Production server started with PID: {}", pid))?;
        }
        Err(_started) => {
            let binary_path = format!("target/release/{}", &config.project_name);
            if std::path::Path::new(&binary_path).exists() {
                std::process::Command::new("script")
                    .args(["-q", "-c", &binary_path, "storage/logs/server.log"])
                    .spawn()?;
                logger::success(&format!("Production server started using compiled binary: {}", binary_path))?;
            } else {
                let cmd = format!("cargo run --release --bin {}", &config.project_name);
                std::process::Command::new("script")
                    .args(["-q", "-c", &cmd, "storage/logs/server.log"])
                    .spawn()?;
                logger::success("Production server started with cargo run --release")?;
                logger::info("Tip: Build with 'cargo build --release' for faster startup next time")?;
            }
        }
    }
    Ok(())
}

fn run_watch(config: &Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    dep_manager.ensure_installed(&["cargo-watch"], true)?;

    logger::info(&format!("Starting watch mode for {}", &config.project_name))?;

    crate::dashboard::stop_server()?;

    let logs_dir = config.project_dir.join("storage").join("logs");
    std::fs::create_dir_all(&logs_dir)?;

    let blast_dir = config.project_dir.join("storage").join("blast");
    std::fs::create_dir_all(&blast_dir)?;

    let server_log_path = logs_dir.join("server.log");

    let _touch = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&server_log_path)?;
    drop(_touch);

    let watch_cmd = format!(
        "nohup script -q -f -c \"cargo watch -x 'run --bin {}'\" storage/logs/server.log </dev/null >/dev/null 2>&1 & echo $!",
        &config.project_name
    );

    let output = std::process::Command::new("bash")
        .args(["-c", &watch_cmd])
        .output()?;

    let pid_str = String::from_utf8_lossy(&output.stdout);
    let pid = pid_str.trim().parse::<u32>().map_err(|e| BlastError::Invalid(e.to_string()))?;

    let pid_file_path = blast_dir.join("server.pid");
    std::fs::write(&pid_file_path, pid.to_string())?;

    let timestamp = chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]");
    let mut server_log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&server_log_path)?;

    writeln!(server_log, "{} Using development configuration", timestamp)?;
    writeln!(server_log, "{} Watch mode started with PID: {}", timestamp, pid)?;

    logger::success(&format!("Watch mode started with PID: {}. Server will restart automatically when code changes.", pid))?;
    Ok(())
}

fn run_check(config: &Config, verbose: bool) -> BlastResult<()> {
    let project_root = config.project_dir.clone();
    let outcome = crate::governor::run_check(&project_root, verbose)?;
    print!("{}", outcome.output);
    std::io::stdout().flush()?;
    if outcome.violation_count > 0 {
        std::process::exit(1);
    }
    Ok(())
}
