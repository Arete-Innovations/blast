use crate::output::{self, DashboardProgress, OutputMode};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// A unified progress manager that provides consistent progress indication across the application
pub struct ProgressManager {
    pub progress_bar: ProgressBar,
    dashboard_progress: Option<DashboardProgress>,
}

impl ProgressManager {
    // Create a new progress manager for operations with known steps
    #[allow(dead_code)]
    pub fn new(steps: u64) -> Self {
        match output::get_output_mode() {
            OutputMode::Stdout => {
                let pb = ProgressBar::new(steps);
                // Simpler template with fixed width and no spinner
                let style = ProgressStyle::default_bar().template("[{bar:40.cyan/blue}] {pos}/{len}").unwrap().progress_chars("#>-");
                pb.set_style(style);
                pb.enable_steady_tick(Duration::from_millis(120));

                Self {
                    progress_bar: pb,
                    dashboard_progress: None,
                }
            }
            OutputMode::LogFile | OutputMode::StdoutAndLogFile => {
                let pb = if output::get_output_mode() == OutputMode::StdoutAndLogFile {
                    let pb = ProgressBar::new(steps);
                    let style = ProgressStyle::default_bar().template("[{bar:40.cyan/blue}] {pos}/{len}").unwrap().progress_chars("#>-");
                    pb.set_style(style);
                    pb.enable_steady_tick(Duration::from_millis(120));
                    pb
                } else {
                    ProgressBar::hidden()
                };

                let dp = DashboardProgress::new_bar(steps);

                Self {
                    progress_bar: pb,
                    dashboard_progress: Some(dp),
                }
            }
        }
    }

    // Create a new progress manager for operations with indefinite length
    pub fn new_spinner() -> Self {
        match output::get_output_mode() {
            OutputMode::Stdout => {
                let pb = ProgressBar::new_spinner();
                // Simpler spinner with no elapsed time
                let style = ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap();
                pb.set_style(style);
                pb.enable_steady_tick(Duration::from_millis(120));

                Self {
                    progress_bar: pb,
                    dashboard_progress: None,
                }
            }
            OutputMode::LogFile | OutputMode::StdoutAndLogFile => {
                let pb = if output::get_output_mode() == OutputMode::StdoutAndLogFile {
                    let pb = ProgressBar::new_spinner();
                    let style = ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap();
                    pb.set_style(style);
                    pb.enable_steady_tick(Duration::from_millis(120));
                    pb
                } else {
                    ProgressBar::hidden()
                };

                let dp = DashboardProgress::new_spinner();

                Self {
                    progress_bar: pb,
                    dashboard_progress: Some(dp),
                }
            }
        }
    }

    // Set the current operation message
    pub fn set_message(&self, msg: &str) {
        // Also log the message to operations log
        if !crate::output::is_quiet_mode() {
            let _ = crate::output::log(&format!("Progress: {}", msg));
        }

        match &self.dashboard_progress {
            Some(dp) => {
                let mut dp = dp.clone();
                dp.set_message(msg);
            }
            None => self.progress_bar.set_message(msg.to_string()),
        }
    }

    // Increment progress
    #[allow(dead_code)]
    pub fn inc(&self, delta: u64) {
        match &self.dashboard_progress {
            Some(dp) => {
                let mut dp = dp.clone();
                dp.inc(delta);
            }
            None => self.progress_bar.inc(delta),
        }
    }

    // Mark operation as completed with success message
    pub fn success(&self, msg: &str) {
        let formatted_msg = format!("\x1b[32m✔\x1b[0m {}", msg);

        // Log the success message to operations log permanently if not in quiet mode
        if !crate::output::is_quiet_mode() {
            let _ = crate::output::log(&format!("Success: {}", msg));
        }

        match &self.dashboard_progress {
            Some(dp) => {
                let mut dp = dp.clone();
                dp.finish_with_message(&formatted_msg);
            }
            None => self.progress_bar.finish_with_message(formatted_msg),
        }
    }

    // Mark operation as failed with error message
    pub fn error(&self, msg: &str) {
        let formatted_msg = format!("\x1b[31m✖\x1b[0m {}", msg);

        // Log the error message to operations log permanently if not in quiet mode
        if !crate::output::is_quiet_mode() {
            let _ = crate::output::log(&format!("Error: {}", msg));
        }

        match &self.dashboard_progress {
            Some(dp) => {
                let mut dp = dp.clone();
                dp.finish_with_message(&formatted_msg);
            }
            None => self.progress_bar.finish_with_message(formatted_msg),
        }
    }
}

// Create a progress manager wrapped in Arc<Mutex<>> for thread-safe sharing
#[allow(dead_code)]
pub fn create_shared_progress(steps: u64) -> Arc<Mutex<ProgressManager>> {
    Arc::new(Mutex::new(ProgressManager::new(steps)))
}

// Create a spinner progress manager wrapped in Arc<Mutex<>> for thread-safe sharing
#[allow(dead_code)]
pub fn create_shared_spinner() -> Arc<Mutex<ProgressManager>> {
    Arc::new(Mutex::new(ProgressManager::new_spinner()))
}
