use crate::error::BlastResult;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct GovernorConfig {
    #[serde(default = "default_rules")]
    pub rules: Vec<String>,
    #[serde(default = "default_exempt_color_files")]
    pub exempt_color_files: Vec<String>,
    #[serde(default = "default_exempt_px_files")]
    pub exempt_px_files: Vec<String>,
    #[serde(default = "default_max_lines_per_sfc")]
    pub max_lines_per_sfc: usize,
    #[serde(default = "default_max_lines_per_fn")]
    pub max_lines_per_fn: usize,
    #[serde(default = "default_whitelist_snippets")]
    pub whitelist_snippets: Vec<String>,
    #[serde(default = "default_icon_class_patterns")]
    pub icon_class_patterns: Vec<String>,
    #[serde(default = "default_scan_globs")]
    pub scan_globs: Vec<String>,
    #[serde(default = "default_hairline_border_rem")]
    pub hairline_border_rem: String,
    #[serde(default = "default_icons_file")]
    pub icons_file: String,
    #[serde(default = "default_tokens_file")]
    pub tokens_file: String,
    #[serde(default = "default_primevue_preset_file")]
    pub primevue_preset_file: String,
}

fn default_rules() -> Vec<String> {
    vec![
        "RawColorOutsidePreset".to_string(),
        "HardcodedPx".to_string(),
        "RawRemOutsideTokens".to_string(),
        "InlineStyle".to_string(),
        "TypeAny".to_string(),
        "TsIgnore".to_string(),
        "SilentFallback".to_string(),
        "ConsoleLog".to_string(),
        "IconClassOutsideIconsFile".to_string(),
        "MaxLinesPerSfc".to_string(),
        "MaxLinesPerFn".to_string(),
        "PrimeVueConfigImportOutsidePresetFile".to_string(),
    ]
}

fn default_exempt_color_files() -> Vec<String> {
    vec!["src/plugins/primevue.ts".to_string()]
}

fn default_exempt_px_files() -> Vec<String> {
    vec![
        "src/plugins/primevue.ts".to_string(),
        "src/styles/tokens.css".to_string(),
        "src/styles/base.css".to_string(),
    ]
}

fn default_max_lines_per_sfc() -> usize {
    600
}

fn default_max_lines_per_fn() -> usize {
    120
}

fn default_whitelist_snippets() -> Vec<String> {
    Vec::new()
}

fn default_icon_class_patterns() -> Vec<String> {
    vec![
        r"\bpi pi-[a-z0-9-]+\b".to_string(),
        r"\bph ph-[a-z0-9-]+\b".to_string(),
        r"\bfa fa-[a-z0-9-]+\b".to_string(),
    ]
}

fn default_scan_globs() -> Vec<String> {
    vec![
        "frontend/src/**/*.ts".to_string(),
        "frontend/src/**/*.vue".to_string(),
        "frontend/src/**/*.css".to_string(),
    ]
}

fn default_hairline_border_rem() -> String {
    "0.0625rem".to_string()
}

fn default_icons_file() -> String {
    "src/icons.ts".to_string()
}

fn default_tokens_file() -> String {
    "src/styles/tokens.css".to_string()
}

fn default_primevue_preset_file() -> String {
    "src/plugins/primevue.ts".to_string()
}

impl Default for GovernorConfig {
    fn default() -> Self {
        Self {
            rules: default_rules(),
            exempt_color_files: default_exempt_color_files(),
            exempt_px_files: default_exempt_px_files(),
            max_lines_per_sfc: default_max_lines_per_sfc(),
            max_lines_per_fn: default_max_lines_per_fn(),
            whitelist_snippets: default_whitelist_snippets(),
            icon_class_patterns: default_icon_class_patterns(),
            scan_globs: default_scan_globs(),
            hairline_border_rem: default_hairline_border_rem(),
            icons_file: default_icons_file(),
            tokens_file: default_tokens_file(),
            primevue_preset_file: default_primevue_preset_file(),
        }
    }
}

impl GovernorConfig {
    pub fn load_or_default(blueprint_ir_path: &Path) -> BlastResult<Self> {
        if !blueprint_ir_path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(blueprint_ir_path)?;
        let parsed: Self = serde_json::from_str(&raw)?;
        Ok(parsed)
    }

    pub fn rule_enabled(&self, rule_name: &str) -> bool {
        self.rules.iter().any(|r| r == rule_name)
    }
}
