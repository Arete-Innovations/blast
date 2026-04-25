use crate::configs::Config;
use crate::error::{BlastError, BlastResult};
use crate::fuses::{remove_fuse, toggle_fuse, FuseInfo};
use crate::io::traits::{SinkExt};
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::*;
use diesel::{PgConnection, RunQueryDsl};
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::{format, Cell, Row, Table};
use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::Duration;

pub enum FusesAction {
    List,
    Toggle { fuse_name: String },
    Run { fuse_name: String },
    ViewLogs { fuse_name: String },
    Remove { fuse_name: String },
    LiveTable,
    Exit,
}

pub enum Outcome {
    Continue,
    Exit,
}

fn establish_connection(config: &Config) -> BlastResult<PgConnection> {
    let current_dir = std::env::current_dir()?;
    std::env::set_current_dir(&config.project_dir)?;

    if let Err(e) = dotenv() {
        drop(e);
    }

    let database_url = std::env::var("DATABASE_URL")?;

    std::env::set_current_dir(current_dir)?;

    Ok(PgConnection::establish(&database_url)?)
}

fn check_fuses_table(conn: &mut PgConnection) -> BlastResult<bool> {
    #[derive(Debug, QueryableByName)]
    struct BoolResult {
        #[diesel(sql_type = Bool)]
        pub exists: bool,
    }

    let results = sql_query(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'fuses') as exists",
    )
    .load::<BoolResult>(conn)?;

    if results.is_empty() {
        Ok(false)
    } else {
        Ok(results[0].exists)
    }
}

fn ensure_fuse_dirs(config: &Config) -> BlastResult<()> {
    let fuse_dir = Path::new(&config.project_dir).join("storage").join("fuses");
    std::fs::create_dir_all(&fuse_dir)?;
    Ok(())
}

fn fetch_fuses(config: &Config) -> BlastResult<Vec<FuseInfo>> {
    ensure_fuse_dirs(config)?;

    let mut conn = establish_connection(config)?;

    if !check_fuses_table(&mut conn)? {
        return Ok(Vec::new());
    }

    Ok(sql_query(
        "SELECT id, name, flow_name, schedule_kind, schedule_spec, \
         enabled, last_run_status, last_error, run_count \
         FROM fuses ORDER BY id",
    )
    .load::<FuseInfo>(&mut conn)?)
}

fn format_job_for_display(job: &FuseInfo) -> String {
    let state = if job.enabled { "[ENABLED ]" } else { "[DISABLED]" };
    let name_display = if job.name.len() > 22 {
        format!("{}...", &job.name[0..19])
    } else {
        job.name.clone()
    };
    format!(
        "{}  {:<22}  {}  {}  runs: {}",
        state, name_display, job.schedule_kind, job.schedule_spec, job.run_count
    )
}

fn pick_fuse_action(
    theme: &ColorfulTheme,
    config: &Config,
) -> BlastResult<FusesAction> {
    let jobs = fetch_fuses(config)?;

    if jobs.is_empty() {
        println!("No fuses to manage.");
        thread::sleep(Duration::from_secs(2));
        return Ok(FusesAction::List);
    }

    let job_displays: Vec<String> = jobs.iter().map(|job| format_job_for_display(job)).collect();

    let job_selection = FuzzySelect::with_theme(theme)
        .with_prompt("Select a fuse to manage")
        .default(0)
        .items(&job_displays)
        .interact()?;

    let selected_job = &jobs[job_selection];

    let fuse_actions = vec![
        format!("{} Fuse", if selected_job.enabled { "Disable" } else { "Enable" }),
        "Run Fuse Now".to_string(),
        "View Logs".to_string(),
        "Remove Fuse".to_string(),
        "Cancel".to_string(),
    ];

    let action_selection = FuzzySelect::with_theme(theme)
        .with_prompt(&format!("Action for fuse '{}'", selected_job.name))
        .default(0)
        .items(&fuse_actions)
        .interact()?;

    match action_selection {
        0 => Ok(FusesAction::Toggle { fuse_name: selected_job.name.clone() }),
        1 => Ok(FusesAction::Run { fuse_name: selected_job.name.clone() }),
        2 => Ok(FusesAction::ViewLogs { fuse_name: selected_job.name.clone() }),
        3 => {
            let confirmed = Confirm::with_theme(theme)
                .with_prompt(format!(
                    "Are you sure you want to remove fuse '{}'?",
                    selected_job.name
                ))
                .default(false)
                .interact()?;
            if confirmed {
                Ok(FusesAction::Remove { fuse_name: selected_job.name.clone() })
            } else {
                Ok(FusesAction::List)
            }
        }
        4 => Ok(FusesAction::List),
        other => Err(BlastError::Invalid(format!("unexpected fuse action index: {}", other))),
    }
}

pub fn pick_action(config: &Config) -> BlastResult<FusesAction> {
    let theme = ColorfulTheme::default();

    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush()?;

    println!("\n{}\n", Style::new().bold().underlined().apply_to("FUSES MANAGER"));

    let jobs = fetch_fuses(config)?;

    if jobs.is_empty() {
        println!("No fuses registered. Run `blast migrate` to create the fuses table.\n");
    } else {
        println!("{} fuses registered.\n", jobs.len());
    }

    let menu_options = vec!["View Live Table", "View and Manage Fuses", "Back to Main Menu"];

    let selection = FuzzySelect::with_theme(&theme)
        .with_prompt("Select an option")
        .default(0)
        .items(&menu_options)
        .interact()?;

    match selection {
        0 => Ok(FusesAction::LiveTable),
        1 => pick_fuse_action(&theme, config),
        2 => Ok(FusesAction::Exit),
        other => Err(BlastError::Invalid(format!("unexpected menu selection: {}", other))),
    }
}

pub fn run(
    action: FusesAction,
    config: &Config,
    sink: &mut dyn crate::io::traits::Sink,
    _progress: &mut dyn crate::io::traits::Progress,
) -> BlastResult<Outcome> {
    match action {
        FusesAction::Exit => Ok(Outcome::Exit),

        FusesAction::List => {
            let jobs = fetch_fuses(config)?;
            if jobs.is_empty() {
                sink.info("No fuses registered.");
            } else {
                sink.info(&format!("{} fuses registered.", jobs.len()));
            }
            Ok(Outcome::Continue)
        }

        FusesAction::LiveTable => {
            display_fuses_table(config)?;
            Ok(Outcome::Continue)
        }

        FusesAction::Toggle { fuse_name } => {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                    .template("{spinner:.green} {msg}")
                    .map_err(|e| BlastError::Invalid(e.to_string()))?,
            );
            pb.set_message(format!("Toggling fuse '{}'...", fuse_name));

            match toggle_fuse(config, &fuse_name) {
                Ok(()) => {
                    pb.finish_with_message(format!("Fuse '{}' toggled successfully", fuse_name));
                    sink.success(&format!("fuse '{}' toggled", fuse_name));
                    thread::sleep(Duration::from_secs(1));
                }
                Err(e) => {
                    pb.finish_with_message(format!("Error: {}", e));
                    sink.error(&format!("toggle failed: {}", e));
                    thread::sleep(Duration::from_secs(2));
                }
            }
            Ok(Outcome::Continue)
        }

        FusesAction::Remove { fuse_name } => {
            let jobs = fetch_fuses(config)?;
            let fuse_id = jobs
                .iter()
                .find(|j| j.name == fuse_name)
                .map(|j| j.id as i32)
                .ok_or_else(|| BlastError::NotFound(format!("fuse '{}' not found", fuse_name)))?;

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                    .template("{spinner:.red} {msg}")
                    .map_err(|e| BlastError::Invalid(e.to_string()))?,
            );
            pb.set_message(format!("Removing fuse '{}'...", fuse_name));

            match remove_fuse(config, fuse_id) {
                Ok(()) => {
                    pb.finish_with_message(format!("Fuse '{}' removed successfully", fuse_name));
                    sink.success(&format!("fuse '{}' removed", fuse_name));
                    thread::sleep(Duration::from_secs(1));
                }
                Err(e) => {
                    pb.finish_with_message(format!("Error: {}", e));
                    sink.error(&format!("remove failed: {}", e));
                    thread::sleep(Duration::from_secs(2));
                }
            }
            Ok(Outcome::Continue)
        }

        FusesAction::Run { fuse_name } => {
            sink.info(&format!("triggering immediate run of fuse '{}'...", fuse_name));
            crate::fuses::run_fuse(config, &fuse_name)?;
            sink.success(&format!("fuse '{}' triggered", fuse_name));
            Ok(Outcome::Continue)
        }

        FusesAction::ViewLogs { fuse_name } => {
            crate::fuses::logs_fuse(config, &fuse_name)?;
            Ok(Outcome::Continue)
        }
    }
}

pub fn run_with_picker(
    config: &Config,
    sink: &mut dyn crate::io::traits::Sink,
    progress: &mut dyn crate::io::traits::Progress,
) -> BlastResult<Outcome> {
    loop {
        let action = pick_action(config)?;
        let outcome = run(action, config, sink, progress)?;

        match outcome {
            Outcome::Exit => return Ok(Outcome::Exit),
            Outcome::Continue => {
                print!("\x1B[2J\x1B[1;1H");
                std::io::stdout().flush()?;
            }
        }
    }
}

pub fn display_fuses_table(config: &Config) -> BlastResult<()> {
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush()?;

    println!("\n{}\n", Style::new().bold().underlined().apply_to("FUSES TABLE (LIVE)"));

    let mut jobs = fetch_fuses(config)?;

    if jobs.is_empty() {
        println!("No fuses registered. Create the fuses table with FUSES_MIGRATION_UP.");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        return Ok(());
    }

    let mut last_refresh = std::time::Instant::now();
    let refresh_interval = Duration::from_secs(1);

    print!("\x1B[s");
    std::io::stdout().flush()?;

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let input_handle = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_line(&mut buf) {
            eprintln!("stdin error: {e}");
            return;
        }
        if let Err(e) = tx.send(()) {
            eprintln!("channel send error: {e}");
        }
    });

    let rows = render_table(&jobs)?;
    for row in &rows {
        println!("{}", row);
    }
    std::io::stdout().flush()?;

    loop {
        if last_refresh.elapsed() >= refresh_interval {
            let updated_jobs_result = fetch_fuses(config);
            match updated_jobs_result {
                Ok(updated_jobs) => {
                    jobs = updated_jobs;
                    last_refresh = std::time::Instant::now();

                    print!("\x1B[u");

                    let rows = render_table(&jobs)?;
                    for row in &rows {
                        println!("{}", row);
                    }
                    std::io::stdout().flush()?;
                }
                Err(e) => {
                    println!("Error refreshing data: {}", e);
                    thread::sleep(Duration::from_secs(1));
                    break;
                }
            }
        }

        match rx.try_recv() {
            Ok(()) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        }
    }

    drop(rx);
    match input_handle.join() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("input thread panicked: {:?}", e);
        }
    }

    Ok(())
}

fn render_table(jobs: &[FuseInfo]) -> BlastResult<Vec<String>> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_BOX_CHARS);

    table.add_row(Row::new(vec![
        Cell::new("ID"),
        Cell::new("Name"),
        Cell::new("Flow"),
        Cell::new("Kind"),
        Cell::new("Spec"),
        Cell::new("Enabled"),
        Cell::new("Status"),
        Cell::new("Runs"),
    ]));

    for job in jobs {
        let enabled_cell = if job.enabled {
            Cell::new("yes").style_spec("Fg=green")
        } else {
            Cell::new("no").style_spec("Fg=yellow")
        };

        let status_cell = match job.last_run_status.as_deref() {
            Some("ok") => Cell::new("ok").style_spec("Fg=green"),
            Some("error") => Cell::new("error").style_spec("Fg=red"),
            Some("running") => Cell::new("running").style_spec("Fg=cyan"),
            Some(other) => Cell::new(other),
            None => Cell::new("-"),
        };

        let name_display = if job.name.len() > 28 {
            format!("{}...", &job.name[0..25])
        } else {
            job.name.clone()
        };

        let flow_display = if job.flow_name.len() > 20 {
            format!("{}...", &job.flow_name[0..17])
        } else {
            job.flow_name.clone()
        };

        table.add_row(Row::new(vec![
            Cell::new(&job.id.to_string()),
            Cell::new(&name_display),
            Cell::new(&flow_display),
            Cell::new(&job.schedule_kind),
            Cell::new(&job.schedule_spec),
            enabled_cell,
            status_cell,
            Cell::new(&job.run_count.to_string()),
        ]));
    }

    let mut output = Vec::new();
    table.print(&mut output)?;

    let text =
        String::from_utf8(output).map_err(|e| BlastError::Invalid(e.to_string()))?;
    Ok(text.lines().map(|line| line.to_string()).collect())
}
