use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkEvent {
    Info(String),
    Warn(String),
    Error(String),
    Success(String),
    Debug(String),
    StructuredDiagnostic { kind: String, fields: Vec<(String, String)> },
}

impl SinkEvent {
    pub fn level(&self) -> SinkLevel {
        match self {
            SinkEvent::Info(_) => SinkLevel::Info,
            SinkEvent::Warn(_) => SinkLevel::Warn,
            SinkEvent::Error(_) => SinkLevel::Error,
            SinkEvent::Success(_) => SinkLevel::Success,
            SinkEvent::Debug(_) => SinkLevel::Debug,
            SinkEvent::StructuredDiagnostic { .. } => SinkLevel::Diagnostic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkLevel {
    Debug,
    Info,
    Warn,
    Error,
    Success,
    Diagnostic,
}

impl SinkLevel {
    pub fn icon(self) -> &'static str {
        match self {
            SinkLevel::Debug => "🔍",
            SinkLevel::Info => "ℹ️",
            SinkLevel::Warn => "⚠️",
            SinkLevel::Error => "❌",
            SinkLevel::Success => "✅",
            SinkLevel::Diagnostic => "📋",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SinkLevel::Debug => "DEBUG",
            SinkLevel::Info => "INFO",
            SinkLevel::Warn => "WARNING",
            SinkLevel::Error => "ERROR",
            SinkLevel::Success => "SUCCESS",
            SinkLevel::Diagnostic => "DIAG",
        }
    }
}

impl fmt::Display for SinkLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    StepStart { label: String },
    StepDone { label: String },
    StepFail { label: String, reason: String },
    Tick { current: u64, total: u64 },
}
