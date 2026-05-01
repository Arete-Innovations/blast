#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Info,
    Success,
    Warning,
    Danger,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertKind::Info => "info",
            AlertKind::Success => "success",
            AlertKind::Warning => "warning",
            AlertKind::Danger => "danger",
        }
    }
}
