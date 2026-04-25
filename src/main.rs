use clap::Parser;
use std::process;

mod arsenal;
mod build;
mod codegen;
mod commands;
mod configs;
mod dashboard;
mod database;
mod dependencies;
mod error;
mod fuses;
mod fuses_tui;
mod gen_migration;
mod gen_picker;
mod gen_table;
mod governor;
mod interactive;
mod io;
mod logger;
mod models;
mod progress;
mod project;
mod schema_parser;
mod structs;
mod tui_viewer;

fn main() {
    let mut dep_manager = dependencies::DependencyManager::new();

    let cli = commands::Cli::parse();

    if cli.verbose {
        std::env::set_var("BLAST_VERBOSE", "1");
    }

    if let Err(e) = logger::init(logger::RuntimeMode::Cli, None) {
        eprintln!("logger init failed: {}", e);
    }

    logger::set_verbose_mode(cli.verbose);

    let cmd = match cli.cmd {
        Some(c) => c,
        None => commands::Command::Dashboard,
    };

    match configs::get_project_info() {
        Ok(mut config) => {
            let interactive = matches!(cmd, commands::Command::Dashboard | commands::Command::Cli);
            if let Err(e) = logger::setup_for_mode(&config, interactive) {
                eprintln!("Warning: Failed to set up logging: {}", e);
            }

            if let Err(e) = commands::execute(cmd, &mut config, &mut dep_manager) {
                eprintln!("Error executing command: {}", e);
                process::exit(1);
            }
        }
        Err(e) => {
            if matches!(cmd, commands::Command::New { .. } | commands::Command::Help) {
                let cwd = match std::env::current_dir() {
                    Ok(c) => c,
                    Err(io_err) => {
                        eprintln!("Failed to get current directory: {}", io_err);
                        process::exit(1);
                    }
                };
                let project_name = match &cmd {
                    commands::Command::New { name, .. } => name.clone(),
                    _other => "unknown".to_string(),
                };
                let mut default_config = configs::Config {
                    environment: "dev".to_string(),
                    project_name,
                    project_dir: cwd,
                    show_compiler_warnings: true,
                    last_modified: std::time::SystemTime::now(),
                };

                if let Err(e) = commands::execute(cmd, &mut default_config, &mut dep_manager) {
                    eprintln!("Error executing command: {}", e);
                    process::exit(1);
                }
            } else {
                eprintln!("Failed to read project info: {}", e);
                eprintln!("You must run this command from a project directory or use 'blast new <project_name>' to create a new project.");
                process::exit(1);
            }
        }
    }
    process::exit(0);
}
