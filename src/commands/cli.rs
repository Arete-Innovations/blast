use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MenuKind {
    /// App menu: watch dev/prod + codegen + db + logs + arsenal + exit.
    App,
    /// Fuses menu: list/toggle/run/logs/live-table — drives the Fuses tab.
    Fuses,
}

#[derive(Debug, Parser)]
#[command(name = "blast", about = "Catalyst generator and workflow CLI", disable_help_subcommand = true)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub cmd: Option<Command>,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum Command {
    #[command(about = "Scaffold a new Catalyst app")]
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
        /// Skip cargo build, cargo leptos build, and TUI launch. For tight iteration loops.
        #[arg(long = "no-warmup")]
        no_warmup: bool,
    },

    #[command(about = "Scaffold a Catalyst app in-place (or in <name>); like `new` but defaults to cwd")]
    Init {
        /// Optional project name. If given, scaffold to `./<name>/`. If
        /// omitted, scaffold directly into the current directory (which
        /// must be empty unless `--force` is set).
        name: Option<String>,
        /// Postgres URL for the new project. If omitted, prompts interactively.
        #[arg(long = "db-url")]
        db_url: Option<String>,
        /// Allow scaffolding into a non-empty directory (or recreate
        /// existing databases).
        #[arg(long)]
        force: bool,
        /// Skip creation of the `<dbname>_test` database and `.env.test` file.
        #[arg(long = "no-test-db")]
        no_test_db: bool,
        /// Skip cargo build, cargo leptos build, and TUI launch. For tight iteration loops.
        #[arg(long = "no-warmup")]
        no_warmup: bool,
    },

    #[command(about = "Create a new Diesel migration skeleton")]
    Migration {
        /// If provided, write an empty migration skeleton with this name and skip the interactive wizard.
        /// Migration name must match snake_case (^[a-z][a-z0-9_]*$). Required for non-TTY contexts (CI, scripts).
        #[arg(long)]
        name: Option<String>,
    },

    #[command(about = "Sync vendored framework code from upstream catalyst into this project")]
    Sync {
        /// Use BLAST_CATALYST_DEV_PATH (local catalyst checkout) instead of git URL.
        #[arg(long)]
        dev: bool,
    },

    #[command(about = "Run pending migrations")]
    Migrate,

    #[command(about = "Roll back migrations")]
    Rollback,

    #[command(about = "Run seed SQL (all or specific file)")]
    Seed { file: Option<String> },

    #[command(about = "Regenerate src/database/schema.rs from DB")]
    Schema,

    #[command(about = "Run dev server (cargo leptos serve — builds + serves BE + WASM bundle)", alias = "serve")]
    Run,

    #[command(name = "run-prod", about = "Run production server (cargo leptos serve --release)", alias = "serve-prod")]
    RunProd,

    #[command(about = "Stop the background dev/prod server daemon")]
    Stop,

    #[command(about = "Watch BE + WASM with cargo leptos watch (live-reload on src changes)")]
    Watch,

    #[command(name = "watch-prod", about = "Watch BE + WASM with cargo leptos watch --release (max-opt wasm + LTO + strip)")]
    WatchProd,

    #[command(name = "e2e", about = "Run end-to-end tests (cargo leptos end-to-end — boots server, runs tests/e2e harness)")]
    E2e,

    #[command(about = "Launch Zellij dashboard")]
    Dashboard,

    #[command(about = "Launch interactive menu (default: app, or `blast cli fuses` for the fuses menu)")]
    Cli {
        #[arg(value_enum, default_value_t = MenuKind::App)]
        menu: MenuKind,
    },

    #[command(name = "toggle-env", about = "Flip Env::Dev <-> Env::Prod", alias = "env")]
    ToggleEnv,

    #[command(about = "Production build via cargo leptos build --release --precompress")]
    Build,

    #[command(about = "Archive release artifact (binary + target/site WASM bundle + .env.example)")]
    Package,

    #[command(about = "Reinstall deps + rerun init pipeline")]
    Refresh,

    #[command(about = "Show top-level help")]
    Help,

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

    #[command(about = "Generate Rust enums from CREATE TYPE statements in migrations")]
    Enums {
        #[arg(value_name = "ENUM_NAME")]
        resource: Option<String>,
    },

    #[command(about = "Generate model implementations")]
    Models,

    #[command(about = "Emit Rust field validators (src/structs/generated/validators)")]
    Validators {
        #[arg(value_name = "RESOURCE")]
        resource: Option<String>,
    },

    #[command(about = "Run the full codegen pipeline (BE + leptos pages/forms/tables/data/app_routes)")]
    All,
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
    use clap::Parser;

    use super::*;

    #[test]
    fn new_command_accepts_minimum_args() {
        let cli = Cli::try_parse_from(["blast", "new", "myapp"]).expect("parse");
        match cli.cmd {
            Some(Command::New {
                name,
                dev,
                db_url,
                force,
                no_test_db,
                no_warmup,
            }) => {
                assert_eq!(name, "myapp");
                assert!(!dev);
                assert!(db_url.is_none());
                assert!(!force);
                assert!(!no_test_db);
                assert!(!no_warmup);
            }
            other => panic!("expected New, got {:?}", other),
        }
    }

    #[test]
    fn new_command_accepts_all_db_args() {
        let cli = Cli::try_parse_from(["blast", "new", "myapp", "--db-url", "postgres://u:p@h/x", "--force", "--no-test-db", "--no-warmup"]).expect("parse");
        match cli.cmd {
            Some(Command::New {
                name,
                dev,
                db_url,
                force,
                no_test_db,
                no_warmup,
            }) => {
                assert_eq!(name, "myapp");
                assert!(!dev);
                assert_eq!(db_url.as_deref(), Some("postgres://u:p@h/x"));
                assert!(force);
                assert!(no_test_db);
                assert!(no_warmup);
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

    #[test]
    fn init_command_no_name_uses_cwd() {
        let cli = Cli::try_parse_from(["blast", "init"]).expect("parse");
        match cli.cmd {
            Some(Command::Init {
                name,
                db_url,
                force,
                no_test_db,
                no_warmup,
            }) => {
                assert!(name.is_none(), "expected no name, got {:?}", name);
                assert!(db_url.is_none());
                assert!(!force);
                assert!(!no_test_db);
                assert!(!no_warmup);
            }
            other => panic!("expected Init, got {:?}", other),
        }
    }

    #[test]
    fn init_command_accepts_name_and_flags() {
        let cli = Cli::try_parse_from(["blast", "init", "myapp", "--db-url", "postgres://u:p@h/x", "--force", "--no-test-db", "--no-warmup"]).expect("parse");
        match cli.cmd {
            Some(Command::Init {
                name,
                db_url,
                force,
                no_test_db,
                no_warmup,
            }) => {
                assert_eq!(name.as_deref(), Some("myapp"));
                assert_eq!(db_url.as_deref(), Some("postgres://u:p@h/x"));
                assert!(force);
                assert!(no_test_db);
                assert!(no_warmup);
            }
            other => panic!("expected Init, got {:?}", other),
        }
    }

    #[test]
    fn migration_command_without_name_runs_wizard() {
        let cli = Cli::try_parse_from(["blast", "migration"]).expect("parse");
        match cli.cmd {
            Some(Command::Migration { name }) => {
                assert!(name.is_none(), "no --name → wizard path; got {:?}", name);
            }
            other => panic!("expected Migration, got {:?}", other),
        }
    }

    #[test]
    fn migration_command_with_name_skips_wizard() {
        let cli = Cli::try_parse_from(["blast", "migration", "--name", "create_widgets"]).expect("parse");
        match cli.cmd {
            Some(Command::Migration { name }) => {
                assert_eq!(name.as_deref(), Some("create_widgets"), "--name <foo> → non-interactive skeleton path");
            }
            other => panic!("expected Migration, got {:?}", other),
        }
    }
}
