use crate::state::names::{FieldName, ResourceName};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A single declared environment variable in the app's env spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVarSpec {
    /// Default value to emit in `.env.example`.  Ignored when `sensitive = true`.
    pub default: String,
    /// Optional human-readable comment rendered above the var line.
    pub comment: Option<String>,
    /// When true the `.env.example` line shows `<NAME>=<changeme>` instead of
    /// the actual default, so secrets are never committed in example files.
    pub sensitive: bool,
}

/// Ordered set of declared env vars.  The key is the env var name (e.g.
/// `DATABASE_URL`).  `IndexMap` preserves insertion order for deterministic
/// file output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvSpecState {
    pub vars: IndexMap<String, EnvVarSpec>,
}

fn default_rules() -> BTreeSet<String> {
    [
        "RawColorOutsidePreset",
        "HardcodedPx",
        "RawRemOutsideTokens",
        "InlineStyle",
        "TypeAny",
        "TsIgnore",
        "SilentFallback",
        "ConsoleLog",
        "IconClassOutsideIconsFile",
        "MaxLinesPerSfc",
        "MaxLinesPerFn",
        "PrimeVueConfigImportOutsidePresetFile",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_exempt_color_files() -> BTreeSet<String> {
    ["src/plugins/primevue.ts"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_exempt_px_files() -> BTreeSet<String> {
    [
        "src/plugins/primevue.ts",
        "src/styles/tokens.css",
        "src/styles/base.css",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_max_lines_per_sfc() -> usize {
    600
}

fn default_max_lines_per_fn() -> usize {
    120
}

fn default_icon_class_patterns() -> BTreeSet<String> {
    [
        r"\bpi pi-[a-z0-9-]+\b",
        r"\bph ph-[a-z0-9-]+\b",
        r"\bfa fa-[a-z0-9-]+\b",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_scan_globs() -> BTreeSet<String> {
    [
        "frontend/src/**/*.ts",
        "frontend/src/**/*.vue",
        "frontend/src/**/*.css",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
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

pub const APP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub schema_version: u32,
    pub sections: IndexMap<String, AppPolicySection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppPolicySection {
    FeLint(FeLintState),
    Admin(AdminState),
    Fuses(FusesPolicyState),
    Services(ServicesState),
    EnvSpec(EnvSpecState),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeLintState {
    pub rules: BTreeSet<String>,
    pub exempt_color_files: BTreeSet<String>,
    pub exempt_px_files: BTreeSet<String>,
    pub max_lines_per_sfc: usize,
    pub max_lines_per_fn: usize,
    pub whitelist_snippets: BTreeSet<String>,
    pub icon_class_patterns: BTreeSet<String>,
    pub scan_globs: BTreeSet<String>,
    pub hairline_border_rem: String,
    pub icons_file: String,
    pub tokens_file: String,
    pub primevue_preset_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminState {
    pub mount_path: String,
    pub actions: IndexMap<ResourceName, BTreeSet<AdminAction>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AdminAction {
    pub slug: String,
    pub label: String,
    pub confirm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusesPolicyState {
    pub entries: IndexMap<String, FuseEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuseEntry {
    pub flow: String,
    pub schedule_cron: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicesState {
    pub storage: ServiceBackend,
    pub email: ServiceBackend,
    pub rate_limit: ServiceBackend,
    pub session_token_ttl_seconds: u64,
    pub admin_scope_fields: BTreeSet<FieldName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceBackend {
    LocalDisk { root: String },
    Smtp { host: String, port: u16 },
    InMemory,
}

impl FeLintState {
    pub fn rule_enabled(&self, rule_name: &str) -> bool {
        self.rules.contains(rule_name)
    }
}

impl Default for FeLintState {
    fn default() -> Self {
        Self {
            rules: default_rules(),
            exempt_color_files: default_exempt_color_files(),
            exempt_px_files: default_exempt_px_files(),
            max_lines_per_sfc: default_max_lines_per_sfc(),
            max_lines_per_fn: default_max_lines_per_fn(),
            whitelist_snippets: BTreeSet::new(),
            icon_class_patterns: default_icon_class_patterns(),
            scan_globs: default_scan_globs(),
            hairline_border_rem: default_hairline_border_rem(),
            icons_file: default_icons_file(),
            tokens_file: default_tokens_file(),
            primevue_preset_file: default_primevue_preset_file(),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            schema_version: APP_SCHEMA_VERSION,
            sections: IndexMap::new(),
        }
    }

    pub fn canonicalize(&mut self) {
        let mut pairs: Vec<(String, AppPolicySection)> = self.sections.drain(..).collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, mut v) in pairs {
            v.canonicalize();
            self.sections.insert(k, v);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppPolicySection {
    pub fn canonicalize(&mut self) {
        match self {
            Self::FeLint(_) => {}
            Self::Admin(state) => {
                let mut pairs: Vec<(ResourceName, BTreeSet<AdminAction>)> =
                    state.actions.drain(..).collect();
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                for (k, v) in pairs {
                    state.actions.insert(k, v);
                }
            }
            Self::Fuses(state) => {
                let mut pairs: Vec<(String, FuseEntry)> = state.entries.drain(..).collect();
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                for (k, v) in pairs {
                    state.entries.insert(k, v);
                }
            }
            Self::Services(_) => {}
            Self::EnvSpec(state) => {
                let mut pairs: Vec<(String, EnvVarSpec)> = state.vars.drain(..).collect();
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                for (k, v) in pairs {
                    state.vars.insert(k, v);
                }
            }
        }
    }
}
