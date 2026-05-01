use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::Local;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::{
    error::BlastResult,
    io::{
        events::{ProgressEvent, SinkEvent, SinkLevel},
        traits::{Progress, Sink},
    },
};

pub struct CliSink {
    out: Box<dyn Write + Send>,
    err: Box<dyn Write + Send>,
    log_file: Option<PathBuf>,
    verbose: bool,
    quiet: bool,
    colorize: bool,
}

pub struct CliSinkConfig {
    pub log_file: Option<PathBuf>,
    pub verbose: bool,
    pub quiet: bool,
    pub colorize: bool,
}

impl Default for CliSinkConfig {
    fn default() -> Self {
        Self {
            log_file: None,
            verbose: false,
            quiet: false,
            colorize: true,
        }
    }
}

impl CliSink {
    pub fn new(cfg: CliSinkConfig) -> Self {
        Self {
            out: Box::new(io::stdout()),
            err: Box::new(io::stderr()),
            log_file: cfg.log_file,
            verbose: cfg.verbose,
            quiet: cfg.quiet,
            colorize: cfg.colorize,
        }
    }

    fn write_log_file(&self, level: SinkLevel, body: &str) -> BlastResult<()> {
        let path = match &self.log_file {
            Some(p) => p,
            None => return Ok(()),
        };
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "[{}] [{}] {}", timestamp, level.label(), body)?;
        Ok(())
    }

    fn render_to_writer(&mut self, level: SinkLevel, body: &str) -> io::Result<()> {
        let icon = level.icon();
        let target_is_err = matches!(level, SinkLevel::Error);
        let line = if self.colorize {
            match level {
                SinkLevel::Info | SinkLevel::Debug => format!("{} {}", icon, body),
                SinkLevel::Warn => format!("{} {}", icon, style(body).yellow()),
                SinkLevel::Error => format!("{} {}", icon, style(body).red().bold()),
                SinkLevel::Success => format!("{} {}", icon, style(body).green()),
            }
        } else {
            format!("{} {}", icon, body)
        };
        let writer: &mut dyn Write = if target_is_err { &mut *self.err } else { &mut *self.out };
        writeln!(writer, "{}", line)?;
        writer.flush()
    }

    fn should_emit(&self, level: SinkLevel) -> bool {
        if self.quiet {
            return false;
        }
        if matches!(level, SinkLevel::Debug) && !self.verbose {
            return false;
        }
        true
    }

    fn handle(&mut self, event: &SinkEvent) -> BlastResult<()> {
        let level = event.level();
        let body = render_body(event);

        self.write_log_file(level, &body)?;

        if !self.should_emit(level) {
            return Ok(());
        }

        match self.render_to_writer(level, &body) {
            Ok(()) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

impl Sink for CliSink {
    fn emit(&mut self, event: SinkEvent) {
        match self.handle(&event) {
            Ok(()) => {}
            Err(err) => {
                let fallback = format!("[blast cli sink failure: {}]", err);
                match writeln!(io::stderr(), "{}", fallback) {
                    Ok(()) => {}
                    Err(io_err) => drop(io_err),
                }
            }
        }
    }
}

pub fn render_body(event: &SinkEvent) -> String {
    match event {
        SinkEvent::Info(msg) | SinkEvent::Warn(msg) | SinkEvent::Error(msg) | SinkEvent::Success(msg) | SinkEvent::Debug(msg) => msg.clone(),
    }
}

#[derive(Clone)]
pub struct CliProgress {
    bar: ProgressBar,
    total: Option<u64>,
    quiet: bool,
}

pub struct CliProgressConfig {
    pub total: Option<u64>,
    pub quiet: bool,
}

impl Default for CliProgressConfig {
    fn default() -> Self {
        Self { total: None, quiet: false }
    }
}

impl CliProgress {
    pub fn new(cfg: CliProgressConfig) -> Self {
        let bar = build_bar(cfg.total);
        Self { bar, total: cfg.total, quiet: cfg.quiet }
    }

    fn step_start(&mut self, label: &str) {
        if self.quiet {
            return;
        }
        self.bar.set_message(label.to_string());
    }

    fn step_done(&mut self, label: &str) {
        if self.quiet {
            return;
        }
        match self.total {
            Some(_total) => {
                self.bar.set_message(label.to_string());
                self.bar.inc(1);
            }
            None => {
                self.bar.finish_and_clear();
                println!("{} {}", SinkLevel::Success.icon(), label);
                self.bar = build_bar(None);
            }
        }
    }

    fn step_fail(&mut self, label: &str, reason: &str) {
        if self.quiet {
            return;
        }
        self.bar.finish_and_clear();
        let formatted = format!("{}: {}", label, reason);
        eprintln!("{} {}", SinkLevel::Error.icon(), style(formatted).red().bold());
        self.bar = build_bar(self.total);
    }

    fn tick(&mut self, current: u64, total: u64) {
        if self.quiet {
            return;
        }
        self.bar.set_length(total);
        self.bar.set_position(current);
        self.total = Some(total);
    }
}

impl Progress for CliProgress {
    fn emit(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::StepStart { label } => self.step_start(&label),
            ProgressEvent::StepDone { label } => self.step_done(&label),
            ProgressEvent::StepFail { label, reason } => self.step_fail(&label, &reason),
            ProgressEvent::Tick { current, total } => self.tick(current, total),
        }
    }
}

fn build_bar(total: Option<u64>) -> ProgressBar {
    match total {
        Some(steps) => {
            let pb = ProgressBar::new(steps);
            let style_result = ProgressStyle::default_bar().template("{spinner:.green} {wide_msg} [{pos}/{len}]");
            let style = match style_result {
                Ok(s) => s.progress_chars("=>-"),
                Err(err) => {
                    drop(err);
                    ProgressStyle::default_bar()
                }
            };
            pb.set_style(style);
            pb
        }
        None => {
            let pb = ProgressBar::new_spinner();
            let style_result = ProgressStyle::default_spinner().template("{spinner:.green} {wide_msg}");
            let style = match style_result {
                Ok(s) => s,
                Err(err) => {
                    drop(err);
                    ProgressStyle::default_spinner()
                }
            };
            pb.set_style(style);
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        }
    }
}

pub fn cli_sink(verbose: bool, log_file: Option<&Path>) -> CliSink {
    CliSink::new(CliSinkConfig {
        log_file: log_file.map(|p| p.to_path_buf()),
        verbose,
        quiet: false,
        colorize: true,
    })
}

pub fn cli_progress(steps: Option<u64>) -> CliProgress {
    CliProgress::new(CliProgressConfig { total: steps, quiet: false })
}
