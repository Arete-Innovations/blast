use std::path::Path;

use super::{
    console_log::ConsoleLog, hardcoded_px::HardcodedPx, hardcoded_route_path::HardcodedRoutePath, icon_class::IconClassOutsideIconsFile, inline_layout_props::InlineLayoutProps, inline_style::InlineStyle,
    loading_spinner::LoadingSpinnerAfterFirstLoad, local_list_state::LocalListState, local_modal_state::LocalModalState, local_storage_outside_persistence::LocalStorageOutsidePersistence,
    max_lines_per_fn::MaxLinesPerFn, max_lines_per_sfc::MaxLinesPerSfc, max_template_depth::MaxTemplateDepth, max_template_loc::MaxTemplateLoc, optimistic_update::OptimisticUpdateInCustom,
    page_shell_required::PageShellRequired, pinia_import::PiniaImport, primevue_config::PrimeVueConfigImportOutsidePresetFile, primevue_reinvented::PrimeVueReinvented, raw_color::RawColorOutsidePreset,
    raw_fetch::RawFetchOutsideApi, raw_rem::RawRemOutsideTokens, silent_fallback::SilentFallback, snake_case_interface_fields::SnakeCaseInterfaceFields, ts_ignore::TsIgnore, type_any::TypeAny,
    websocket_outside_relay::WebSocketOutsideRelay,
};
use crate::{
    governor::{
        rules::traits::{FileRule, Rule},
        violation::Violation,
    },
    state::FeLintState,
};

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
        Box::new(SnakeCaseInterfaceFields::new()),
    ]
}
