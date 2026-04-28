use std::path::Path;

use crate::{
    governor::{
        rules::{
            helpers::{is_comment_line, rel_path_str, snippet_of},
            traits::Rule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

pub struct PrimeVueConfigImportOutsidePresetFile;

impl PrimeVueConfigImportOutsidePresetFile {
    pub fn new() -> Self {
        Self
    }
}

fn is_preset_file(file: &Path, preset_path: &str) -> bool {
    let path = rel_path_str(file);
    let normalized = preset_path.replace('\\', "/");
    path.ends_with(&normalized)
}

fn line_imports_primevue_config(line: &str) -> bool {
    let stripped = line.trim_start();
    if !stripped.starts_with("import ") && !stripped.contains(" from ") && !line.contains("require(") {
        return false;
    }
    line.contains("primevue.config") || line.contains("PrimeVueConfig") || line.contains("primevue/config")
}

impl Rule for PrimeVueConfigImportOutsidePresetFile {
    fn name(&self) -> &'static str {
        "PrimeVueConfigImportOutsidePresetFile"
    }

    fn check(&self, file: &Path, line: &str, line_no: usize, config: &FeLintState) -> Option<Violation> {
        if is_preset_file(file, &config.primevue_preset_file) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        if !line_imports_primevue_config(line) {
            return None;
        }
        Some(Violation::new(
            "PrimeVueConfigImportOutsidePresetFile",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "PrimeVue config belongs only in src/plugins/primevue.ts",
        ))
    }
}
