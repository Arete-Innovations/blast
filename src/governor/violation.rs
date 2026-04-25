use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Violation {
    pub rule: String,
    pub file: PathBuf,
    pub line_no: usize,
    pub snippet: String,
    pub suggestion: String,
}

impl Violation {
    pub fn new(
        rule: impl Into<String>,
        file: PathBuf,
        line_no: usize,
        snippet: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            rule: rule.into(),
            file,
            line_no,
            snippet: snippet.into(),
            suggestion: suggestion.into(),
        }
    }
}
