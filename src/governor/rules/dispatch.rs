use crate::governor::rules::traits::{FileRule, Rule};
use crate::state::FeLintState;
use crate::governor::violation::Violation;
use std::path::Path;

use super::console_log::ConsoleLog;
use super::hardcoded_px::HardcodedPx;
use super::hardcoded_route_path::HardcodedRoutePath;
use super::icon_class::IconClassOutsideIconsFile;
use super::inline_layout_props::InlineLayoutProps;
use super::inline_style::InlineStyle;
use super::loading_spinner::LoadingSpinnerAfterFirstLoad;
use super::local_list_state::LocalListState;
use super::local_modal_state::LocalModalState;
use super::local_storage_outside_persistence::LocalStorageOutsidePersistence;
use super::max_lines_per_fn::MaxLinesPerFn;
use super::max_lines_per_sfc::MaxLinesPerSfc;
use super::max_template_depth::MaxTemplateDepth;
use super::max_template_loc::MaxTemplateLoc;
use super::optimistic_update::OptimisticUpdateInCustom;
use super::page_shell_required::PageShellRequired;
use super::pinia_import::PiniaImport;
use super::primevue_config::PrimeVueConfigImportOutsidePresetFile;
use super::primevue_reinvented::PrimeVueReinvented;
use super::raw_color::RawColorOutsidePreset;
use super::raw_fetch::RawFetchOutsideApi;
use super::raw_rem::RawRemOutsideTokens;
use super::silent_fallback::SilentFallback;
use super::ts_ignore::TsIgnore;
use super::type_any::TypeAny;
use super::websocket_outside_relay::WebSocketOutsideRelay;

pub fn run_all(file: &Path, contents: &str, config: &FeLintState) -> Vec<Violation> {
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
        Box::new(HardcodedRoutePath::new()),
        Box::new(LocalListState::new()),
        Box::new(LoadingSpinnerAfterFirstLoad::new()),
        Box::new(RawFetchOutsideApi::new()),
        Box::new(WebSocketOutsideRelay::new()),
        Box::new(LocalStorageOutsidePersistence::new()),
        Box::new(PiniaImport::new()),
    ]
}

fn build_file_rules() -> Vec<Box<dyn FileRule>> {
    vec![
        Box::new(MaxLinesPerSfc::new()),
        Box::new(MaxLinesPerFn::new()),
        Box::new(LocalModalState::new()),
        Box::new(OptimisticUpdateInCustom::new()),
        Box::new(PageShellRequired::new()),
        Box::new(InlineLayoutProps::new()),
        Box::new(MaxTemplateDepth::new()),
        Box::new(MaxTemplateLoc::new()),
        Box::new(PrimeVueReinvented::new()),
    ]
}
