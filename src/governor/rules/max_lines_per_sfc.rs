use std::path::Path;

use crate::{
    governor::{
        rules::{
            helpers::{extension_is, snippet_of},
            traits::FileRule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

pub struct MaxLinesPerSfc;

impl MaxLinesPerSfc {
    pub fn new() -> Self {
        Self
    }
}

impl FileRule for MaxLinesPerSfc {
    fn name(&self) -> &'static str {
        "MaxLinesPerSfc"
    }

    fn check_file(&self, file: &Path, contents: &str, config: &FeLintState) -> Vec<Violation> {
        if !extension_is(file, "vue") {
            return Vec::new();
        }
        let line_count = contents.lines().count();
        if line_count <= config.max_lines_per_sfc {
            return Vec::new();
        }
        let snippet = format!("SFC has {} lines", line_count);
        let suggestion = format!("split this SFC; max is {} lines", config.max_lines_per_sfc);
        vec![Violation::new("MaxLinesPerSfc", file.to_path_buf(), line_count, snippet_of(&snippet), suggestion)]
    }
}
