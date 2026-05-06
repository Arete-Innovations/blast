use std::collections::BTreeSet;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::state::{
    names::{FieldName, ResourceName},
    resource::SoftDeleteDefault,
};

// ── Nav + Pages types ────────────────────────────────────────────────────────

/// Layout variants for a page shell.  Maps to `<PageShell layout="...">` in
/// generated Vue routes.  PascalCase because RON serialises bare enum variants
/// that way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageLayout {
    Cards,
    Split,
    Table,
    Bleed,
    Tabbed,
}

/// Role variants for auth-gating of routes and nav entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Admin,
}

/// A single nav menu item that references a named route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Route name. Must resolve to an auto-emitted CRUD route or a declared
    /// `Page.route`.  Codegen fails on dangling references.
    pub route: String,
    /// Optional override label for this entry. Falls back to the referenced
    /// `Page.label` (or the route name) at codegen time when None.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional icon registry key override.  Falls back to the referenced
    /// `Page.icon` at codegen time when None.
    #[serde(default)]
    pub icon: Option<String>,
    /// Optional per-entry visibility restriction.  Must be a subset of the
    /// referenced route's own role requirement.
    #[serde(default)]
    pub roles: Option<Vec<Role>>,
}

/// A top-level nav menu group containing one or more `Entry` items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    /// Stable identifier used for active-route highlighting.
    pub key: String,
    /// Human-readable group label.
    pub label: String,
    /// Icon registry key (resolves to `IC.<icon>` from `src/icons.ts`).
    pub icon: String,
    /// If set, the entire section is hidden for roles not in this list.
    #[serde(default)]
    pub roles: Option<Vec<Role>>,
    /// Ordered menu items inside this section.
    pub entries: Vec<Entry>,
}

/// Top-level nav configuration: an ordered list of menu sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavConfig {
    pub sections: Vec<Section>,
}

/// A custom (non-CRUD) page route declared in the Blueprint.
///
/// CRUD routes are auto-emitted from each Primer's verb list; those do NOT
/// need to appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// Route name. Becomes part of the leptos route registration emitted
    /// by `app_routes` codegen. Dot-notation convention: `dashboard`,
    /// `audit.detail`.
    pub route: String,
    /// URL path. Supports leptos_router param syntax (`/foo/:id`).
    /// No trailing slash.
    pub path: String,
    /// Component identifier emitted by codegen — module path under
    /// src/transport/leptos/pages plus the component type name, joined
    /// by a colon (consumed verbatim by the app_routes runner).
    pub component: String,
    /// Page shell layout variant.
    pub layout: PageLayout,
    /// Human-readable name used in nav and breadcrumbs.
    #[serde(default)]
    pub label: Option<String>,
    /// Icon registry key.
    #[serde(default)]
    pub icon: Option<String>,
    /// When false the route is reachable but not auto-included in any menu.
    pub in_nav: bool,
    /// Auth-gating roles.  Codegen emits both router-guard check and
    /// menu-visibility check.
    #[serde(default)]
    pub roles: Option<Vec<Role>>,
}

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

pub const APP_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub schema_version: u32,
    pub sections: IndexMap<String, AppPolicySection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppPolicySection {
    Admin(AdminState),
    Fuses(FusesPolicyState),
    Services(ServicesState),
    EnvSpec(EnvSpecState),
    Defaults(DefaultsState),
    Nav(NavConfig),
    Pages(Vec<Page>),
    Sync(SyncConfig),
}

/// `blast sync` policy. Lets the user pin specific paths against vendored
/// overwrite — sync skips frozen entries, `blast sync diff` surfaces
/// drift between the local file and what catalyst currently ships so the
/// user can reconcile by hand.
///
/// Freeze entries are project-root-relative paths. A directory entry
/// freezes everything beneath it (prefix match with `/` boundary). A
/// file entry freezes that one file (exact match).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub freeze: Vec<String>,
}

/// App-wide defaults consumed by Blast's TUI as prefill values when
/// scaffolding new resources or fields. These are NOT enforced at
/// codegen time — they only steer wizards.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultsState {
    /// Default soft-delete behavior offered when `gen resource` creates
    /// a new resource. `None` means the wizard prompts with no prefill.
    #[serde(default)]
    pub soft_delete_new_resources_default: Option<SoftDeleteDefault>,
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

    /// Pull the freeze list from the `Sync` section, if present. Returns
    /// an empty vec when no Sync section is declared. "no Sync section"
    /// is a valid state (project hasn't opted into freeze), not a failure.
    pub fn freeze_list(&self) -> Vec<String> {
        match self.sections.get("sync") {
            Some(AppPolicySection::Sync(s)) => s.freeze.clone(),
            Some(_) => Vec::new(), // allow: malformed key: section under "sync" isn't a Sync variant — treat as no freeze
            None => Vec::new(),    // allow: absence of section means no freeze policy declared, not an error
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
            Self::Admin(state) => {
                let mut pairs: Vec<(ResourceName, BTreeSet<AdminAction>)> = state.actions.drain(..).collect();
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
            Self::Defaults(_) => {}
            Self::Nav(_) => {}
            Self::Pages(_) => {}
            Self::Sync(state) => {
                state.freeze.sort();
                state.freeze.dedup();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::upgraders::upgrade_app;

    /// Helper: round-trip a value through RON serialization.
    fn ron_roundtrip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let config = ron::ser::PrettyConfig::new().depth_limit(64).indentor("  ".to_string()).struct_names(true);
        let s = ron::ser::to_string_pretty(value, config).unwrap_or_else(|e| panic!("serialize failed: {e}\nvalue: {value:?}"));
        ron::from_str::<T>(&s).unwrap_or_else(|e| panic!("deserialize failed: {e}\nRON:\n{s}"))
    }

    // ── PageLayout round-trips ────────────────────────────────────────────────

    #[test]
    fn page_layout_cards_roundtrips() {
        let v = PageLayout::Cards;
        assert_eq!(ron_roundtrip(&v), v);
    }

    #[test]
    fn page_layout_split_roundtrips() {
        let v = PageLayout::Split;
        assert_eq!(ron_roundtrip(&v), v);
    }

    #[test]
    fn page_layout_table_roundtrips() {
        let v = PageLayout::Table;
        assert_eq!(ron_roundtrip(&v), v);
    }

    #[test]
    fn page_layout_bleed_roundtrips() {
        let v = PageLayout::Bleed;
        assert_eq!(ron_roundtrip(&v), v);
    }

    #[test]
    fn page_layout_tabbed_roundtrips() {
        let v = PageLayout::Tabbed;
        assert_eq!(ron_roundtrip(&v), v);
    }

    // ── Role round-trips ─────────────────────────────────────────────────────

    #[test]
    fn role_user_roundtrips() {
        let v = Role::User;
        assert_eq!(ron_roundtrip(&v), v);
    }

    #[test]
    fn role_admin_roundtrips() {
        let v = Role::Admin;
        assert_eq!(ron_roundtrip(&v), v);
    }

    // ── AppState default behaviour ────────────────────────────────────────────

    #[test]
    fn default_app_state_roundtrips() {
        let state = AppState::default();
        assert_eq!(ron_roundtrip(&state), state);
    }

    #[test]
    fn default_app_state_has_no_nav_or_pages() {
        let state = AppState::default();
        // Neither "nav" nor "pages" sections are present by default.
        assert!(!state.sections.contains_key("nav"));
        assert!(!state.sections.contains_key("pages"));
    }

    // ── v3 full round-trip ────────────────────────────────────────────────────

    fn make_v3_state() -> AppState {
        let mut state = AppState::new();
        let nav = NavConfig {
            sections: vec![
                Section {
                    key: "main".to_string(),
                    label: "Main".to_string(),
                    icon: "home".to_string(),
                    roles: None,
                    entries: vec![
                        Entry {
                            route: "dashboard".to_string(),
                            label: None,
                            icon: None,
                            roles: None,
                        },
                        Entry {
                            route: "users.list".to_string(),
                            label: None,
                            icon: None,
                            roles: Some(vec![Role::Admin]),
                        },
                    ],
                },
                Section {
                    key: "ops".to_string(),
                    label: "Operations".to_string(),
                    icon: "tools".to_string(),
                    roles: Some(vec![Role::Admin]),
                    entries: vec![Entry {
                        route: "fuses.list".to_string(),
                        label: None,
                        icon: None,
                        roles: None,
                    }],
                },
            ],
        };
        let pages = vec![
            Page {
                route: "dashboard".to_string(),
                path: "/".to_string(),
                component: "custom/pages/DashboardPage.vue".to_string(),
                layout: PageLayout::Cards,
                label: Some("Dashboard".to_string()),
                icon: Some("dashboard".to_string()),
                in_nav: true,
                roles: Some(vec![Role::User, Role::Admin]),
            },
            Page {
                route: "settings".to_string(),
                path: "/settings".to_string(),
                component: "custom/pages/SettingsPage.vue".to_string(),
                layout: PageLayout::Cards,
                label: Some("Settings".to_string()),
                icon: Some("cog".to_string()),
                in_nav: true,
                roles: Some(vec![Role::User, Role::Admin]),
            },
            Page {
                route: "debug.thing".to_string(),
                path: "/_debug/thing".to_string(),
                component: "custom/pages/DebugThing.vue".to_string(),
                layout: PageLayout::Bleed,
                label: None,
                icon: None,
                in_nav: false,
                roles: Some(vec![Role::Admin]),
            },
        ];
        state.sections.insert("nav".to_string(), AppPolicySection::Nav(nav));
        state.sections.insert("pages".to_string(), AppPolicySection::Pages(pages));
        state
    }

    #[test]
    fn v3_full_roundtrip() {
        let original = make_v3_state();
        let after_one = ron_roundtrip(&original);
        assert_eq!(original, after_one);
        let after_two = ron_roundtrip(&after_one);
        assert_eq!(original, after_two);
    }

    #[test]
    fn v3_nav_sections_preserved() {
        let state = ron_roundtrip(&make_v3_state());
        let nav = match state.sections.get("nav") {
            Some(AppPolicySection::Nav(n)) => n.clone(),
            other => panic!("expected Nav section, got {other:?}"),
        };
        assert_eq!(nav.sections.len(), 2);
        assert_eq!(nav.sections[0].key, "main");
        assert_eq!(nav.sections[1].key, "ops");
        assert_eq!(nav.sections[1].roles, Some(vec![Role::Admin]));
    }

    #[test]
    fn v3_pages_preserved() {
        let state = ron_roundtrip(&make_v3_state());
        let pages = match state.sections.get("pages") {
            Some(AppPolicySection::Pages(p)) => p.clone(),
            other => panic!("expected Pages section, got {other:?}"),
        };
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].route, "dashboard");
        assert_eq!(pages[0].layout, PageLayout::Cards);
        assert!(pages[0].in_nav);
        assert_eq!(pages[2].route, "debug.thing");
        assert!(!pages[2].in_nav);
        assert_eq!(pages[2].label, None);
    }

    #[test]
    fn v2_upgrades_to_current() {
        let mut state = AppState {
            schema_version: 2,
            sections: IndexMap::new(),
        };
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
        assert!(!state.sections.contains_key("nav"));
        assert!(!state.sections.contains_key("pages"));
    }

    #[test]
    fn v1_upgrades_to_current() {
        let mut state = AppState {
            schema_version: 1,
            sections: IndexMap::new(),
        };
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
    }

    #[test]
    fn current_state_upgrade_is_noop() {
        let mut state = AppState::new();
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
    }

    #[test]
    fn v3_upgrades_to_current() {
        let mut state = AppState {
            schema_version: 3,
            sections: IndexMap::new(),
        };
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
    }

    #[test]
    fn sync_section_round_trips() {
        let mut state = AppState::new();
        let cfg = SyncConfig {
            freeze: vec!["src/views/components/vendored/public_shell.rs".to_string(), "src/style/tokens.scss".to_string()],
        };
        state.sections.insert("sync".to_string(), AppPolicySection::Sync(cfg.clone()));
        let after = ron_roundtrip(&state);
        match after.sections.get("sync") {
            Some(AppPolicySection::Sync(c)) => assert_eq!(c, &cfg),
            other => panic!("expected Sync section, got {other:?}"),
        }
    }

    #[test]
    fn freeze_list_accessor_returns_entries() {
        let mut state = AppState::new();
        let cfg = SyncConfig {
            freeze: vec!["a".to_string(), "b".to_string()],
        };
        state.sections.insert("sync".to_string(), AppPolicySection::Sync(cfg));
        assert_eq!(state.freeze_list(), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn freeze_list_accessor_empty_when_no_section() {
        let state = AppState::new();
        assert!(state.freeze_list().is_empty());
    }

    #[test]
    fn sync_canonicalize_dedups_and_sorts() {
        let mut state = AppState::new();
        let cfg = SyncConfig {
            freeze: vec!["b".to_string(), "a".to_string(), "b".to_string()],
        };
        state.sections.insert("sync".to_string(), AppPolicySection::Sync(cfg));
        state.canonicalize();
        match state.sections.get("sync") {
            Some(AppPolicySection::Sync(c)) => assert_eq!(c.freeze, vec!["a".to_string(), "b".to_string()]),
            other => panic!("expected Sync section, got {other:?}"),
        }
    }
}
