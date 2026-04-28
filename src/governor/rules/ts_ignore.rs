use std::path::Path;

use crate::{
    governor::{
        rules::{helpers::snippet_of, traits::Rule},
        violation::Violation,
    },
    state::FeLintState,
};

pub struct TsIgnore;

impl TsIgnore {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for TsIgnore {
    fn name(&self) -> &'static str {
        "TsIgnore"
    }

    fn check(&self, file: &Path, line: &str, line_no: usize, _config: &FeLintState) -> Option<Violation> {
        if !line.contains("@ts-ignore") && !line.contains("@ts-nocheck") {
            return None;
        }
        Some(Violation::new(
            "TsIgnore",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "fix the underlying type error or use `@allow-any` (discouraged)",
        ))
    }
}
