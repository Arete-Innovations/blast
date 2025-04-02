// This is a stub file that redirects to the new logger module.
// It exists only for backward compatibility during the transition.

use crate::logger;

// Redirect to the new Progress struct
pub struct ProgressManager {
    progress: logger::Progress,
}

impl ProgressManager {
    pub fn new(steps: u64) -> Self {
        Self {
            progress: logger::create_progress(Some(steps)),
        }
    }

    pub fn new_spinner() -> Self {
        Self { progress: logger::create_progress(None) }
    }

    pub fn set_message(&self, msg: &str) {
        let mut progress = self.progress.clone();
        progress.set_message(msg);
    }

    #[allow(dead_code)]
    pub fn inc(&self, delta: u64) {
        let mut progress = self.progress.clone();
        progress.inc(delta);
    }

    pub fn success(&self, msg: &str) {
        let mut progress = self.progress.clone();
        progress.success(msg);
    }

    pub fn error(&self, msg: &str) {
        let mut progress = self.progress.clone();
        progress.error(msg);
    }
}

// Stub functions for backward compatibility
#[allow(dead_code)]
pub fn create_shared_progress(steps: u64) -> std::sync::Arc<std::sync::Mutex<ProgressManager>> {
    std::sync::Arc::new(std::sync::Mutex::new(ProgressManager::new(steps)))
}

#[allow(dead_code)]
pub fn create_shared_spinner() -> std::sync::Arc<std::sync::Mutex<ProgressManager>> {
    std::sync::Arc::new(std::sync::Mutex::new(ProgressManager::new_spinner()))
}
