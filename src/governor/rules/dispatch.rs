use crate::governor::config::GovernorConfig;
use crate::governor::rules::traits::{FileRule, Rule};
use crate::governor::violation::Violation;
use std::path::Path;

use super::console_log::ConsoleLog;
use super::hardcoded_px::HardcodedPx;
use super::icon_class::IconClassOutsideIconsFile;
use super::inline_style::InlineStyle;
use super::max_lines_per_fn::MaxLinesPerFn;
use super::max_lines_per_sfc::MaxLinesPerSfc;
use super::primevue_config::PrimeVueConfigImportOutsidePresetFile;
use super::raw_color::RawColorOutsidePreset;
use super::raw_rem::RawRemOutsideTokens;
use super::silent_fallback::SilentFallback;
use super::ts_ignore::TsIgnore;
use super::type_any::TypeAny;

pub fn run_all(file: &Path, contents: &str, config: &GovernorConfig) -> Vec<Violation> {
    let line_rules = build_line_rules();
    let file_rules = build_file_rules();

    let mut out: Vec<Violation> = Vec::new();

    for (idx, line) in contents.lines().enumerate() {
        let line_no = idx + 1;
        for rule in &line_rules {
            if !config.rule_enabled(rule.name()) {
                continue;
            }
            match rule.check(file, line, line_no, config) {
                Some(v) => out.push(v),
                None => {}
            }
        }
    }

    for rule in &file_rules {
        if !config.rule_enabled(rule.name()) {
            continue;
        }
        let mut found = rule.check_file(file, contents, config);
        out.append(&mut found);
    }

    out
}

fn build_line_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(RawColorOutsidePreset::new()),
        Box::new(HardcodedPx::new()),
        Box::new(RawRemOutsideTokens::new()),
        Box::new(InlineStyle::new()),
        Box::new(TypeAny::new()),
        Box::new(TsIgnore::new()),
        Box::new(SilentFallback::new()),
        Box::new(ConsoleLog::new()),
        Box::new(IconClassOutsideIconsFile::new()),
        Box::new(PrimeVueConfigImportOutsidePresetFile::new()),
    ]
}

fn build_file_rules() -> Vec<Box<dyn FileRule>> {
    vec![
        Box::new(MaxLinesPerSfc::new()),
        Box::new(MaxLinesPerFn::new()),
    ]
}
