use crate::configs::Config;
use crate::error::BlastResult;
use crate::fuses::{add_fuse, remove_fuse, toggle_fuse, FuseInfo};
use chrono::{Local, TimeZone, Utc};
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input};
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

fn format_duration(seconds: i32) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        format!("{}h {}m", hours, mins)
    } else {
        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        format!("{}d {}h", days, hours)
    }
}

fn format_timestamp(timestamp: Option<i64>) -> String {
    match timestamp {
        Some(ts) => match Local.timestamp_opt(ts, 0).single() {
            Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => "Invalid timestamp".to_string(),
        },
        None => "Never".to_string(),
    }
}

fn calc_next_run(last_run: Option<i64>, timer: i32) -> String {
    match last_run {
        Some(ts) => {
            let next_ts = ts + timer as i64;
            let now = Utc::now().timestamp();

            if next_ts <= now {
                "Pending execution".to_string()
            } else {
                let time_left = next_ts - now;
                format_duration(time_left as i32)
            }
        }
        None => "ASAP".to_string(),
    }
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

fn ensure_fuses_table(conn: &mut PgConnection) -> BlastResult<()> {
    if !check_fuses_table(conn)? {
        sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS fuses (
                id SERIAL PRIMARY KEY,
                name VARCHAR NOT NULL UNIQUE,
                timer INT NOT NULL,
                status VARCHAR NOT NULL DEFAULT 'active',
                last_run BIGINT
            );

            CREATE INDEX IF NOT EXISTS idx_fuses_name ON fuses(name);
        "#,
        )
        .execute(conn)?;

        sql_query(
            r#"
            INSERT INTO fuses (name, timer, status)
            VALUES
                ('cleanup_temp_files', 3600, 'active'),
                ('send_digest_emails', 86400, 'active'),
                ('update_search_index', 43200, 'paused')
            ON CONFLICT DO NOTHING;
        "#,
        )
        .execute(conn)?;
    }

    Ok(())
}

fn ensure_fuse_dirs(config: &Config) -> BlastResult<()> {
    let fuse_dir = Path::new(&config.project_dir).join("storage").join("fuses");
    std::fs::create_dir_all(&fuse_dir)?;
    Ok(())
}

fn fetch_fuses(config: &Config) -> BlastResult<Vec<FuseInfo>> {
    ensure_fuse_dirs(config)?;

    let mut conn = establish_connection(config)?;

    ensure_fuses_table(&mut conn)?;

    Ok(sql_query("SELECT id, name, timer, status, last_run FROM fuses ORDER BY id")
        .load::<FuseInfo>(&mut conn)?)
}

pub fn display_fuses_table(config: &Config) -> BlastResult<()> {
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush()?;

    println!("\n{}\n", Style::new().bold().underlined().apply_to("FUSES TABLE (LIVE)"));

    let mut jobs = fetch_fuses(config)?;

    if jobs.is_empty() {
        println!("No fuses registered.");
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
        Cell::new("Status"),
        Cell::new("Interval"),
        Cell::new("Last Run"),
        Cell::new("Next Run"),
    ]));

    for job in jobs {
        let status_cell = match job.status.as_str() {
            "active" => Cell::new(&job.status).style_spec("Fg=green"),
            "paused" => Cell::new(&job.status).style_spec("Fg=yellow"),
            "completed" => Cell::new(&job.status).style_spec("Fg=blue"),
            "failed" => Cell::new(&job.status).style_spec("Fg=red"),
            other => Cell::new(other),
        };

        let name_display = if job.name.len() > 25 {
            format!("{}...", &job.name[0..22])
        } else {
            job.name.clone()
        };

        let last_run = format_timestamp(job.last_run);
        let next_run = calc_next_run(job.last_run, job.timer);
        let interval = format_duration(job.timer);

        let padded_next_run = format!("{:<20}", next_run);

        table.add_row(Row::new(vec![
            Cell::new(&job.id.to_string()),
            Cell::new(&name_display),
            status_cell,
            Cell::new(&interval),
            Cell::new(&last_run),
            Cell::new(&padded_next_run),
        ]));
    }

    let mut output = Vec::new();
    table.print(&mut output)?;

    let text =
        String::from_utf8(output).map_err(|e| crate::error::BlastError::Invalid(e.to_string()))?;
    Ok(text.lines().map(|line| line.to_string()).collect())
}

pub fn run_fuses_tui(config: &Config) -> BlastResult<()> {
    let theme = ColorfulTheme::default();

    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush()?;

    loop {
        println!("\n{}\n", Style::new().bold().underlined().apply_to("FUSES MANAGER"));

        let jobs = fetch_fuses(config)?;

        let format_job_for_display = |job: &FuseInfo| -> String {
            let interval = format_duration(job.timer);
            let status = match job.status.as_str() {
                "active" => "Active",
                "paused" => "Paused",
                "completed" => "Completed",
                "failed" => "Failed",
                other => other,
            };

            let name_display = if job.name.len() > 18 {
                format!("{}...", &job.name[0..15])
            } else {
                job.name.clone()
            };

            format!(
                "ID: {:<3} - {:<18} (Status: {:<12}, Interval: {:<12})",
                job.id, name_display, status, interval
            )
        };

        if jobs.is_empty() {
            println!("No fuses registered.\n");
        } else {
            println!("{} fuses registered.\n", jobs.len());
        }

        let menu_options =
            vec!["View Live Table", "View and Manage Fuses", "Add New Fuse", "Back to Main Menu"];

        let selection = FuzzySelect::with_theme(&theme)
            .with_prompt("Select an option")
            .default(0)
            .items(&menu_options)
            .interact()?;

        match selection {
            0 => {
                display_fuses_table(config)?;
            }
            1 => {
                if jobs.is_empty() {
                    println!("No fuses to manage. Please add a fuse first.");
                    thread::sleep(Duration::from_secs(2));
                } else {
                    let job_displays: Vec<String> =
                        jobs.iter().map(|job| format_job_for_display(job)).collect();

                    let job_selection = FuzzySelect::with_theme(&theme)
                        .with_prompt("Select a fuse to manage")
                        .default(0)
                        .items(&job_displays)
                        .interact()?;

                    let selected_job = &jobs[job_selection];

                    let fuse_actions = vec![
                        format!(
                            "{} Fuse",
                            if selected_job.status == "active" { "Pause" } else { "Activate" }
                        ),
                        "Remove Fuse".to_string(),
                        "Cancel".to_string(),
                    ];

                    let action_selection = FuzzySelect::with_theme(&theme)
                        .with_prompt(&format!("Action for fuse '{}'", selected_job.name))
                        .default(0)
                        .items(&fuse_actions)
                        .interact()?;

                    match action_selection {
                        0 => {
                            let pb = ProgressBar::new_spinner();
                            pb.set_style(
                                ProgressStyle::default_spinner()
                                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                                    .template("{spinner:.green} {msg}")
                                    .map_err(|e| {
                                        crate::error::BlastError::Invalid(e.to_string())
                                    })?,
                            );
                            pb.set_message(format!("Toggling fuse '{}'...", selected_job.name));

                            match toggle_fuse(config, selected_job.id) {
                                Ok(()) => {
                                    pb.finish_with_message(format!(
                                        "Fuse '{}' toggled successfully",
                                        selected_job.name
                                    ));
                                    thread::sleep(Duration::from_secs(1));
                                }
                                Err(e) => {
                                    pb.finish_with_message(format!("Error: {}", e));
                                    thread::sleep(Duration::from_secs(2));
                                }
                            }
                        }
                        1 => {
                            if Confirm::with_theme(&theme)
                                .with_prompt(format!(
                                    "Are you sure you want to remove fuse '{}'?",
                                    selected_job.name
                                ))
                                .default(false)
                                .interact()?
                            {
                                let pb = ProgressBar::new_spinner();
                                pb.set_style(
                                    ProgressStyle::default_spinner()
                                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                                        .template("{spinner:.red} {msg}")
                                        .map_err(|e| {
                                            crate::error::BlastError::Invalid(e.to_string())
                                        })?,
                                );
                                pb.set_message(format!("Removing fuse '{}'...", selected_job.name));

                                match remove_fuse(config, selected_job.id) {
                                    Ok(()) => {
                                        pb.finish_with_message(format!(
                                            "Fuse '{}' removed successfully",
                                            selected_job.name
                                        ));
                                        thread::sleep(Duration::from_secs(1));
                                    }
                                    Err(e) => {
                                        pb.finish_with_message(format!("Error: {}", e));
                                        thread::sleep(Duration::from_secs(2));
                                    }
                                }
                            }
                        }
                        2 => {}
                        other => {
                            eprintln!("unexpected action: {}", other);
                        }
                    }
                }
            }
            2 => {
                let name: String =
                    Input::with_theme(&theme).with_prompt("Enter fuse name").interact_text()?;

                if name.trim().is_empty() {
                    println!("Fuse name cannot be empty.");
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }

                let interval: String = Input::with_theme(&theme)
                    .with_prompt("Enter interval in seconds (e.g. 3600 for hourly)")
                    .default("3600".into())
                    .interact_text()?;

                match interval.parse::<i32>() {
                    Ok(interval_seconds) if interval_seconds > 0 => {
                        let pb = ProgressBar::new_spinner();
                        pb.set_style(
                            ProgressStyle::default_spinner()
                                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                                .template("{spinner:.green} {msg}")
                                .map_err(|e| crate::error::BlastError::Invalid(e.to_string()))?,
                        );
                        pb.set_message(format!("Adding fuse '{}'...", name));

                        match add_fuse(config, &name, interval_seconds) {
                            Ok(()) => {
                                pb.finish_with_message(format!(
                                    "Fuse '{}' added successfully",
                                    name
                                ));
                                thread::sleep(Duration::from_secs(1));
                            }
                            Err(e) => {
                                pb.finish_with_message(format!("Error: {}", e));
                                thread::sleep(Duration::from_secs(2));
                            }
                        }
                    }
                    Ok(interval_seconds) => {
                        println!("Interval must be positive, got {}.", interval_seconds);
                        thread::sleep(Duration::from_secs(2));
                    }
                    Err(e) => {
                        println!("Please enter a valid number for the interval: {}", e);
                        thread::sleep(Duration::from_secs(2));
                    }
                }
            }
            3 => break,
            other => {
                eprintln!("unexpected menu selection: {}", other);
                break;
            }
        }

        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush()?;
    }

    Ok(())
}
