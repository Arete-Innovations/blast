use crate::commands::Command;
use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::logger;
use console::Style;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use std::env;
use std::io::Write;

// Main interactive CLI function
pub fn run_interactive_cli(mut config: Config, dep_manager: &mut DependencyManager) -> Result<(), String> {
    // Set up logging for interactive mode
    logger::setup_for_mode(&config, true)?;

    // Set the environment variable to indicate we're in interactive mode
    env::set_var("BLAST_INTERACTIVE", "1");

    // Clear screen
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().map_err(|e| e.to_string())?;

    // Define the menu items
    let commands = vec![
        // APP commands first (most important)
        "[APP] Refresh",
        "[APP] Run Server",
        "[APP] Launch Dashboard",
        "[APP] Toggle Dev/Prod",
        // Code generation group
        "[CODEGEN] Schema",
        "[CODEGEN] Structs",
        "[CODEGEN] Models",
        // DB commands
        "[DB] New Migration",
        "[DB] Migrate",
        "[DB] Rollback",
        "[DB] Seed",
        // Assets management
        "[Assets] Transpile SCSS",
        "[Assets] Minify CSS",
        "[Assets] Publish CSS",
        "[Assets] Publish JS",
        "[Assets] Download CDN",
        // Log management
        "[LOG] Truncate Logs",
        // Exit is always last
        "[Exit] Kill Session",
    ];

    // Initialize console styles
    let prod_style = Style::new().bold().fg(console::Color::Green);
    let dev_style = Style::new().bold().fg(console::Color::Yellow);

    loop {
        // First, reload the config if it's been modified
        let _ = config.reload_if_modified();

        // Clear screen before showing menu
        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush().map_err(|e| e.to_string())?;

        // Create prompt based on environment
        let prompt = if config.environment == "prod" {
            format!("{}->[{}] ", prod_style.apply_to(format!("[🚀{}]", config.environment.to_uppercase())), config.project_name)
        } else {
            format!("{}->[{}] ", dev_style.apply_to(format!("[🔧{}]", config.environment.to_uppercase())), config.project_name)
        };

        // Show the menu
        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .items(&commands)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;

        // Convert selection to command
        let cmd = match commands[selection] {
            "[APP] Refresh" => Command::RefreshApp,
            "[APP] Run Server" => {
                if config.environment == "prod" || config.environment == "production" {
                    Command::RunProdServer
                } else {
                    Command::RunDevServer
                }
            }
            "[APP] Launch Dashboard" => Command::LaunchDashboard,
            "[APP] Toggle Dev/Prod" => Command::ToggleEnvironment,

            "[CODEGEN] Schema" => Command::GenerateSchema,
            "[CODEGEN] Structs" => Command::GenerateStructs,
            "[CODEGEN] Models" => Command::GenerateModels,

            "[DB] New Migration" => Command::NewMigration,
            "[DB] Migrate" => Command::Migrate,
            "[DB] Rollback" => Command::Rollback,
            "[DB] Seed" => Command::Seed(None),

            "[Assets] Transpile SCSS" => Command::TranspileScss,
            "[Assets] Minify CSS" => Command::MinifyCss,
            "[Assets] Publish CSS" => Command::PublishCss,
            "[Assets] Publish JS" => Command::ProcessJs,
            "[Assets] Download CDN" => Command::DownloadCdn,

            "[LOG] Truncate Logs" => Command::LogTruncate(None),

            "[Exit] Kill Session" => {
                // Log the exit
                logger::info("Killing Zellij session...")?;

                // Try to use zellij to exit the session
                let _ = std::process::Command::new("zellij").args(["kill-session"]).spawn();

                // If that doesn't work, kill all sessions
                let _ = std::process::Command::new("zellij").args(["kill-all-sessions", "-y"]).spawn();

                Command::Exit
            }
            _ => continue,
        };

        // Exit early if Exit command
        if cmd == Command::Exit {
            break;
        }

        // Clear screen before executing command
        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush().map_err(|e| e.to_string())?;

        // Execute the command
        match crate::commands::execute(cmd, &mut config, dep_manager) {
            Ok(_) => {
                // Success message already logged by command handler
                // Sleep briefly to make sure user sees any output
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            Err(e) => {
                logger::error(&format!("Command failed: {}", e))?;

                // Make sure the user sees the error
                println!("\nPress Enter to continue...");
                let mut buffer = String::new();
                std::io::stdin().read_line(&mut buffer).map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}
