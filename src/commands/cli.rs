use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "blast", about = "Catablast generator and workflow CLI", disable_help_subcommand = true)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub cmd: Option<Command>,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum Command {
    #[command(about = "Scaffold a new Catablast app")]
    New {
        name: String,
        #[arg(long)]
        dev: bool,
        /// Postgres URL for the new project. If omitted, prompts interactively.
        #[arg(long = "db-url")]
        db_url: Option<String>,
        /// Drop and recreate target databases if they exist with tables.
        #[arg(long)]
        force: bool,
        /// Skip creation of the `<dbname>_test` database and `.env.test` file.
        #[arg(long = "no-test-db")]
        no_test_db: bool,
    },

    #[command(about = "Initialize project (migrations, schema, codegen)")]
    Init,

    #[command(about = "Create a new Diesel migration skeleton")]
    Migration,

    #[command(about = "Run pending migrations")]
    Migrate,

    #[command(about = "Roll back migrations")]
    Rollback,

    #[command(about = "Run seed SQL (all or specific file)")]
    Seed { file: Option<String> },

    #[command(about = "Regenerate src/database/schema.rs from DB")]
    Schema,

    #[command(about = "Run dev server", alias = "serve")]
    Run,

    #[command(name = "run-prod", about = "Run production server", alias = "serve-prod")]
    RunProd,

    #[command(about = "Stop background blast run process")]
    Stop,

    #[command(about = "Watch backend with cargo-watch")]
    Watch,

    #[command(about = "Launch Zellij dashboard")]
    Dashboard,

    #[command(about = "Launch dialoguer interactive menu")]
    Cli,

    #[command(name = "toggle-env", about = "Flip Env::Dev <-> Env::Prod", alias = "env")]
    ToggleEnv,

    #[command(about = "Production build")]
    Build,

    #[command(about = "Archive release artifact")]
    Package,

    #[command(about = "Reinstall deps + rerun init pipeline")]
    Refresh,

    #[command(about = "Show top-level help")]
    Help,

    #[command(about = "Run frontend lint engine (Governor)")]
    Check {
        #[arg(long)]
        verbose: bool,
    },

    #[command(about = "Manage scheduler fuses")]
    Fuses {
        #[command(subcommand)]
        cmd: Option<FusesCmd>,
    },

    #[command(about = "Code generation targets")]
    Gen {
        #[command(subcommand)]
        cmd: Option<GenCmd>,
    },

    #[command(about = "Manage blast log files", alias = "logs")]
    Log {
        #[command(subcommand)]
        cmd: LogCmd,
    },

    #[command(about = "Capability inventory tool")]
    Arsenal {
        #[command(subcommand)]
        cmd: Option<ArsenalCmd>,
    },
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum FusesCmd {
    #[command(about = "List all registered fuses")]
    List,

    #[command(about = "Toggle a fuse's enabled flag")]
    Toggle { name: String },

    #[command(about = "Trigger immediate run of a fuse")]
    Run { name: String },

    #[command(about = "Show recent run logs for a fuse")]
    Logs { name: String },

    #[command(about = "Launch interactive fuses TUI", alias = "tui")]
    Interactive,

    #[command(name = "live-table", about = "Auto-refreshing fuses table view", alias = "live")]
    LiveTable,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum GenCmd {
    #[command(about = "Generate structs from schema")]
    Structs,

    #[command(about = "Generate model implementations")]
    Models,

    #[command(about = "Interactive CREATE TABLE wizard")]
    Table,

    #[command(about = "Scaffold an empty migration and open $EDITOR")]
    Migration {
        #[arg(long)]
        custom: bool,
        name: Option<String>,
    },

    #[command(about = "Generate frontend artifacts from primer IR")]
    Frontend,

    #[command(name = "governor-plugin", about = "Emit governor Vite plugin shim")]
    GovernorPlugin,

    #[command(name = "fe-scaffold", about = "Seed tokens.css, base.css, primevue.ts")]
    FeScaffold,

    #[command(about = "Scaffold per-flow + per-route test stubs")]
    Test {
        #[arg(long, conflicts_with = "route")]
        flow: Option<String>,
        #[arg(long, conflicts_with = "flow")]
        route: Option<String>,
    },

    #[command(about = "Run the full codegen pipeline (schema → structs → models → flows → frontend → governor → tests)")]
    All,

    #[command(about = "TUI wizard to author/edit a resource state file")]
    Resource { name: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum LogCmd {
    #[command(about = "Truncate log files (all or specific)")]
    Truncate { file: Option<String> },

    #[command(about = "Interactive TUI log viewer")]
    View { level: String },
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum ArsenalCmd {
    #[command(about = "Serve capability inventory over MCP stdio")]
    Serve,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn new_command_accepts_minimum_args() {
        let cli = Cli::try_parse_from(["blast", "new", "myapp"]).expect("parse");
        match cli.cmd {
            Some(Command::New { name, dev, db_url, force, no_test_db }) => {
                assert_eq!(name, "myapp");
                assert!(!dev);
                assert!(db_url.is_none());
                assert!(!force);
                assert!(!no_test_db);
            }
            other => panic!("expected New, got {:?}", other),
        }
    }

    #[test]
    fn new_command_accepts_all_db_args() {
        let cli = Cli::try_parse_from([
            "blast",
            "new",
            "myapp",
            "--db-url",
            "postgres://u:p@h/x",
            "--force",
            "--no-test-db",
        ])
        .expect("parse");
        match cli.cmd {
            Some(Command::New { name, dev, db_url, force, no_test_db }) => {
                assert_eq!(name, "myapp");
                assert!(!dev);
                assert_eq!(db_url.as_deref(), Some("postgres://u:p@h/x"));
                assert!(force);
                assert!(no_test_db);
            }
            other => panic!("expected New, got {:?}", other),
        }
    }

    #[test]
    fn new_command_dev_flag_still_works() {
        let cli = Cli::try_parse_from(["blast", "new", "myapp", "--dev"]).expect("parse");
        match cli.cmd {
            Some(Command::New { dev, .. }) => assert!(dev),
            other => panic!("expected New, got {:?}", other),
        }
    }
}
