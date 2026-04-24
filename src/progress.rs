use crate::logger;

pub struct ProgressManager {
    progress: logger::Progress,
}

impl ProgressManager {
    pub fn new_spinner() -> Self {
        Self { progress: logger::create_progress(None) }
    }

    pub fn set_message(&self, msg: &str) {
        let mut progress = self.progress.clone();
        progress.set_message(msg);
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
