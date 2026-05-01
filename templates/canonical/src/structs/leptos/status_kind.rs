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

    pub fn variant(&self) -> &'static str {
        match self {
            StatusKind::Online => "success",
            StatusKind::Offline => "info",
            StatusKind::Pending => "brand",
            StatusKind::Error => "danger",
        }
    }
}

impl std::fmt::Display for StatusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            StatusKind::Online => "Online",
            StatusKind::Offline => "Offline",
            StatusKind::Pending => "Pending",
            StatusKind::Error => "Error",
        })
    }
}
