#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Online,
    Offline,
    Pending,
    Error,
}

impl StatusKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StatusKind::Online => "online",
            StatusKind::Offline => "offline",
            StatusKind::Pending => "pending",
            StatusKind::Error => "error",
        }
    }
}
