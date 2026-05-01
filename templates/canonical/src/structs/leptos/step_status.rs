#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepStatus {
    #[default]
    Pending,
    Active,
    Done,
    Error,
}

impl StepStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            StepStatus::Pending => "pending",
            StepStatus::Active => "active",
            StepStatus::Done => "done",
            StepStatus::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepItem {
    pub label: String,
    pub status: StepStatus,
}

impl StepItem {
    pub fn new(label: impl Into<String>, status: StepStatus) -> Self {
        Self {
            label: label.into(),
            status,
        }
    }
}
