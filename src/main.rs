use dotenv::dotenv;
use std::env;
use std::process;

mod assets;
mod commands;
mod configs;
mod dashboard;
mod database;
mod dependencies;
mod git;
mod interactive;
mod locale;
mod models;
mod output;
mod progress;
mod project;
mod structs;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file if present
    dotenv().ok();

    // Create dependency manager
    let dep_manager = dependencies::DependencyManager::new();

    // Get command line arguments
    let args: Vec<String> = env::args().collect();

    // Set output mode to stdout by default for CLI operations
    output::set_output_mode(output::OutputMode::Stdout);

    // Set operation context to CLI by default
    output::set_operation_context("CLI");

    // Handle CLI mode if arguments are provided
    if args.len() > 1 {
        match commands::parse_cli_args(&args) {
            Some(commands::Action::NewProject(_)) => {
                if let Err(e) = commands::execute_action(commands::Action::NewProject(args[2].clone()), None, &dep_manager).await {
                    eprintln!("Error creating project: {}", e);
                    process::exit(1);
                }
                process::exit(0);
            }
            Some(commands::Action::Help) => {
                commands::show_help();
                process::exit(0);
            }
            Some(action) => {
                // Load project config for other commands
                match configs::get_project_info() {
                    Ok(mut config) => {
                        if let Err(e) = commands::execute_action(action, Some(&mut config), &dep_manager).await {
                            eprintln!("Error executing command: {}", e);
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read project info: {}", e);
                        process::exit(1);
                    }
                }
                process::exit(0);
            }
            None => {
                eprintln!("Unknown command. Run 'blast help' for usage information.");
                process::exit(1);
            }
        }
    }

    // If no arguments provided, launch dashboard by default
    match configs::get_project_info() {
        Ok(config) => {
            // Launch dashboard directly
            if let Err(e) = commands::execute_action(commands::Action::LaunchDashboard, Some(&mut config.clone()), &dep_manager).await {
                eprintln!("Error launching dashboard: {}", e);
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to read project info: {}", e);
            process::exit(1);
        }
    }
}
