use std::collections::BTreeSet;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::state::{
    icons::IconConfig,
    names::{FieldName, ResourceName},
    resource::SoftDeleteDefault,
    theme::ThemeConfig,
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
    /// Route name.  Becomes part of the `RouteName` union in
    /// `frontend/src/generated/router/route-names.ts`.
    /// Dot-notation convention: `dashboard`, `audit.detail`.
    pub route: String,
    /// URL path.  Supports vue-router param syntax (`/foo/:id`).
    /// No trailing slash.
    pub path: String,
    /// Path to the hand-written Vue component, relative to `frontend/src/`
    /// (e.g. `custom/pages/DashboardPage.vue`).
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
        "MaxTemplateDepth",
        "MaxTemplateLoc",
        "PrimeVueConfigImportOutsidePresetFile",
        "HardcodedRoutePath",
        "LocalModalState",
        "LocalListState",
        "OptimisticUpdateInCustom",
        "PageShellRequired",
        "InlineLayoutProps",
        "LoadingSpinnerAfterFirstLoad",
        "RawFetchOutsideApi",
        "WebSocketOutsideRelay",
        "LocalStorageOutsidePersistence",
        "PiniaImport",
        "PrimeVueReinvented",
        "SnakeCaseInterfaceFields",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_exempt_color_files() -> BTreeSet<String> {
    ["src/plugins/primevue.ts"].iter().map(|s| s.to_string()).collect()
}

fn default_exempt_px_files() -> BTreeSet<String> {
    ["src/plugins/primevue.ts", "src/styles/tokens.css", "src/styles/base.css"].iter().map(|s| s.to_string()).collect()
}

fn default_max_lines_per_sfc() -> usize {
    600
}

fn default_max_lines_per_fn() -> usize {
    120
}

fn default_max_template_depth() -> u32 {
    5
}

fn default_max_template_loc() -> u32 {
    200
}

fn default_icon_class_patterns() -> BTreeSet<String> {
    [r"\bpi pi-[a-z0-9-]+\b", r"\bph ph-[a-z0-9-]+\b", r"\bfa fa-[a-z0-9-]+\b"].iter().map(|s| s.to_string()).collect()
}

fn default_scan_globs() -> BTreeSet<String> {
    ["frontend/src/**/*.ts", "frontend/src/**/*.vue", "frontend/src/**/*.css"].iter().map(|s| s.to_string()).collect()
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

pub const APP_SCHEMA_VERSION: u32 = 4;

/// Section key under which the design-token + PrimeVue preset config is
/// stored inside `AppState.sections`.
pub const THEME_SECTION_KEY: &str = "theme";

/// Section key under which the icon registry is stored inside
/// `AppState.sections`.
pub const ICONS_SECTION_KEY: &str = "icons";

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
    Defaults(DefaultsState),
    /// Navigation menu tree (`nav` section in `app.ron`).
    Nav(NavConfig),
    /// Custom (non-CRUD) page routes (`pages` section in `app.ron`).
    Pages(Vec<Page>),
    /// Design tokens + PrimeVue palette preset. Drives codegen of
    /// `frontend/src/styles/tokens.css` and `frontend/src/plugins/primevue.ts`.
    Theme(ThemeConfig),
    /// Icon registry. Drives codegen of `frontend/src/icons.ts`.
    Icons(IconConfig),
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
pub struct FeLintState {
    pub rules: BTreeSet<String>,
    pub exempt_color_files: BTreeSet<String>,
    pub exempt_px_files: BTreeSet<String>,
    pub max_lines_per_sfc: usize,
    pub max_lines_per_fn: usize,
    #[serde(default = "default_max_template_depth")]
    pub max_template_depth: u32,
    #[serde(default = "default_max_template_loc")]
    pub max_template_loc: u32,
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
            max_template_depth: default_max_template_depth(),
            max_template_loc: default_max_template_loc(),
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
            // Theme + Icons hold BTreeMap-backed registries that are
            // already canonical-by-construction.
            Self::Theme(_) => {}
            Self::Icons(_) => {}
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
        // We want a synthetic v3-shaped state for upgrade tests; the
        // constructor uses the live APP_SCHEMA_VERSION which is now v4,
        // so override to v3 explicitly.
        state.schema_version = 3;
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
                            roles: None,
                        },
                        Entry {
                            route: "users.list".to_string(),
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

    // ── Multi-step upgrade: v(N) → v(current) ───────────────────────────────
    //
    // Each upgrader is purely additive in this codebase, so a v1 or v2
    // state walks through every step and ends at APP_SCHEMA_VERSION
    // with the v4 default theme + icons sections injected (since neither
    // was present in older revs) and still no nav / pages (those are
    // user-opt-in additions that v2→v3 does NOT inject).

    #[test]
    fn v2_upgrades_to_current_with_default_theme_and_icons() {
        let mut state = AppState {
            schema_version: 2,
            sections: IndexMap::new(),
        };
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
        // v2→v3 is a no-op for sections.
        assert!(!state.sections.contains_key("nav"));
        assert!(!state.sections.contains_key("pages"));
        // v3→v4 injects theme + icons defaults.
        assert!(state.sections.contains_key("theme"));
        assert!(state.sections.contains_key("icons"));
    }

    #[test]
    fn v1_upgrades_to_current_with_default_theme_and_icons() {
        let mut state = AppState {
            schema_version: 1,
            sections: IndexMap::new(),
        };
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
        assert!(!state.sections.contains_key("nav"));
        assert!(!state.sections.contains_key("pages"));
        assert!(state.sections.contains_key("theme"));
        assert!(state.sections.contains_key("icons"));
    }

    // ── v3 → v4 upgrade: dedicated coverage ───────────────────────────────────

    #[test]
    fn v3_upgrades_to_v4_injects_theme_and_icons_defaults() {
        let mut state = AppState {
            schema_version: 3,
            sections: IndexMap::new(),
        };
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
        assert_eq!(APP_SCHEMA_VERSION, 4);

        let theme = match state.sections.get("theme") {
            Some(AppPolicySection::Theme(t)) => t.clone(),
            other => panic!("expected Theme section, got {other:?}"),
        };
        // Defaults round-trip through ThemeConfig::default — sample a few
        // load-bearing fields:
        assert_eq!(theme.primevue.primary.palette.palette, "violet");
        assert!(theme.tokens.font_sizes.len() >= 10);
        assert!(theme.tokens.spacing.len() >= 14);

        let icons = match state.sections.get("icons") {
            Some(AppPolicySection::Icons(i)) => i.clone(),
            other => panic!("expected Icons section, got {other:?}"),
        };
        assert_eq!(icons.registry.len(), 21);
    }

    #[test]
    fn v3_upgrade_preserves_user_supplied_theme() {
        // If the user has already added a theme section to their v3 file
        // (e.g. via hand-edit), v3 → v4 must NOT clobber it with defaults.
        let mut custom_theme = crate::state::theme::ThemeConfig::default();
        // mutate one observable field so we can detect overwrite
        custom_theme.primevue.primary.palette = crate::state::theme::PaletteRef::new("emerald");

        let mut state = AppState {
            schema_version: 3,
            sections: IndexMap::new(),
        };
        state.sections.insert("theme".to_string(), AppPolicySection::Theme(custom_theme.clone()));

        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);

        let theme = match state.sections.get("theme") {
            Some(AppPolicySection::Theme(t)) => t.clone(),
            other => panic!("expected preserved Theme section, got {other:?}"),
        };
        assert_eq!(theme.primevue.primary.palette.palette, "emerald");
        // icons still got the default since it was absent
        assert!(state.sections.contains_key("icons"));
    }

    #[test]
    fn v3_upgrade_preserves_user_supplied_icons() {
        // Mirror of the above for the icons section.
        let mut state = AppState {
            schema_version: 3,
            sections: IndexMap::new(),
        };
        let mut registry = std::collections::BTreeMap::new();
        let key = crate::state::icons::IconKey::new("custom-key").expect("test key");
        let class = crate::state::icons::IconClass::new("pi pi-custom").expect("test class");
        registry.insert(key.clone(), class);
        state.sections.insert("icons".to_string(), AppPolicySection::Icons(crate::state::icons::IconConfig { registry }));

        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);

        let icons = match state.sections.get("icons") {
            Some(AppPolicySection::Icons(i)) => i.clone(),
            other => panic!("expected preserved Icons section, got {other:?}"),
        };
        assert_eq!(icons.registry.len(), 1);
        assert!(icons.registry.contains_key(&key));
    }

    #[test]
    fn v4_state_upgrade_is_noop() {
        // A state already at the current version walks zero upgraders.
        let mut state = AppState::new();
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
        upgrade_app(&mut state).expect("upgrade_app should succeed");
        assert_eq!(state.schema_version, APP_SCHEMA_VERSION);
    }

    #[test]
    fn v4_full_roundtrip_with_theme_and_icons() {
        // Build a v4 state that exercises both new sections and round-trip
        // through RON twice.
        let mut state = AppState::new();
        state.sections.insert("theme".to_string(), AppPolicySection::Theme(crate::state::theme::ThemeConfig::default()));
        state.sections.insert("icons".to_string(), AppPolicySection::Icons(crate::state::icons::IconConfig::default()));

        let after_one = ron_roundtrip(&state);
        assert_eq!(state, after_one);
        let after_two = ron_roundtrip(&after_one);
        assert_eq!(state, after_two);
    }
}
