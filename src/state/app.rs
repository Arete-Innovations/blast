use crate::state::names::{FieldName, ResourceName};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
        }
    }
}
