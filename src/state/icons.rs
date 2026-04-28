//! Icon registry state.
//!
//! `IconConfig` is the state-side source of truth for the FE file
//! `frontend/src/icons.ts` — a typed registry mapping a friendly icon
//! name (e.g. `home`, `dashboard`) to the underlying CSS class string
//! (`pi pi-home`, `pi pi-th-large`).
//!
//! The Governor lint rule `IconClassOutsideIconsFile` enforces that no
//! literal `"pi pi-foo"` appears outside this generated file. Components
//! reference `IC.<name>` and the constant carries an `as const` so the
//! `IconName` type is exact.
//!
//! Defaults here MUST round-trip to byte-identical output against the
//! current static `ICONS_TS` constant in `src/codegen/fe_runtime_extras.rs`.
//! Wave B's codegen lane consumes this catalog and emits the file.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{BlastError, BlastResult};

/// Friendly identifier-like icon key. Validated to be `[a-z][a-z0-9_-]*`
/// so it can safely appear as a TypeScript object literal property
/// without quoting and as a JS identifier in `IC.<name>` access.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IconKey(pub String);

impl IconKey {
    pub fn new<S: Into<String>>(s: S) -> BlastResult<Self> {
        let s = s.into();
        if s.is_empty() {
            return Err(BlastError::Invalid("icon key must be non-empty".into()));
        }
        let mut chars = s.chars();
        let first = match chars.next() {
            Some(c) => c,
            None => return Err(BlastError::Invalid("icon key must have a first char".into())),
        };
        if !first.is_ascii_lowercase() {
            return Err(BlastError::Invalid(format!("icon key must start with [a-z], got {:?}", s)));
        }
        for c in chars {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
                return Err(BlastError::Invalid(format!("icon key {:?} contains invalid char {:?}", s, c)));
            }
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// PrimeIcons CSS class string. Validated to start with the `"pi pi-"`
/// prefix to keep the registry honest about its source library — if you
/// want a Phosphor or Font Awesome icon, that requires an explicit
/// extension to this type, not silent acceptance of a different prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconClass(pub String);

impl IconClass {
    pub const PRIMEICONS_PREFIX: &'static str = "pi pi-";

    pub fn new<S: Into<String>>(s: S) -> BlastResult<Self> {
        let s = s.into();
        if !s.starts_with(Self::PRIMEICONS_PREFIX) {
            return Err(BlastError::Invalid(format!("icon class must start with {:?}, got {:?}", Self::PRIMEICONS_PREFIX, s)));
        }
        let suffix = &s[Self::PRIMEICONS_PREFIX.len()..];
        if suffix.is_empty() {
            return Err(BlastError::Invalid(format!("icon class {:?} has empty icon name", s)));
        }
        for c in suffix.chars() {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(BlastError::Invalid(format!("icon class {:?} suffix has invalid char {:?}", s, c)));
            }
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Registry of friendly icon name → PrimeIcons class string. Iterated
/// in `BTreeMap` order so codegen output is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconConfig {
    pub registry: BTreeMap<IconKey, IconClass>,
}

impl Default for IconConfig {
    fn default() -> Self {
        let pairs: &[(&str, &str)] = &[
            // Navigation / common
            ("home", "pi pi-home"),
            ("dashboard", "pi pi-th-large"),
            ("settings", "pi pi-cog"),
            ("cog", "pi pi-cog"),
            ("user", "pi pi-user"),
            ("users", "pi pi-users"),
            ("tools", "pi pi-wrench"),
            // Actions
            ("add", "pi pi-plus"),
            ("edit", "pi pi-pencil"),
            ("delete", "pi pi-trash"),
            ("save", "pi pi-check"),
            ("cancel", "pi pi-times"),
            ("back", "pi pi-arrow-left"),
            // Status
            ("warning", "pi pi-exclamation-triangle"),
            ("error", "pi pi-times-circle"),
            ("success", "pi pi-check-circle"),
            ("info", "pi pi-info-circle"),
            // Data
            ("search", "pi pi-search"),
            ("filter", "pi pi-filter"),
            ("sort", "pi pi-sort"),
            ("refresh", "pi pi-refresh"),
        ];
        let mut registry = BTreeMap::new();
        for (k, v) in pairs {
            // Default literals are author-controlled and validated by
            // round-trip tests; constructor failure here is a build-time
            // bug, not a runtime input bug.
            let key = IconKey::new(*k).expect("default icon key literal validates"); // allow: hard-coded literal in default impl
            let class = IconClass::new(*v).expect("default icon class literal validates"); // allow: hard-coded literal in default impl
            registry.insert(key, class);
        }
        Self { registry }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ron_roundtrip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let config = ron::ser::PrettyConfig::new().depth_limit(64).indentor("  ".to_string()).struct_names(true);
        let s = ron::ser::to_string_pretty(value, config).unwrap_or_else(|e| panic!("serialize failed: {e}\nvalue: {value:?}"));
        ron::from_str::<T>(&s).unwrap_or_else(|e| panic!("deserialize failed: {e}\nRON:\n{s}"))
    }

    #[test]
    fn icon_key_accepts_simple_identifiers() {
        assert!(IconKey::new("home").is_ok());
        assert!(IconKey::new("foo_bar").is_ok());
        assert!(IconKey::new("foo-bar").is_ok());
        assert!(IconKey::new("a1").is_ok());
    }

    #[test]
    fn icon_key_rejects_invalid() {
        assert!(IconKey::new("").is_err());
        assert!(IconKey::new("Home").is_err()); // upper
        assert!(IconKey::new("1home").is_err()); // leading digit
        assert!(IconKey::new("ho me").is_err()); // space
    }

    #[test]
    fn icon_class_requires_pi_prefix() {
        assert!(IconClass::new("pi pi-home").is_ok());
        assert!(IconClass::new("pi pi-arrow-left").is_ok());
        assert!(IconClass::new("ph ph-home").is_err());
        assert!(IconClass::new("pi pi-").is_err()); // empty suffix
        assert!(IconClass::new("pi-home").is_err()); // missing space
    }

    #[test]
    fn default_icon_config_round_trips_through_ron() {
        let cfg = IconConfig::default();
        let after = ron_roundtrip(&cfg);
        assert_eq!(cfg, after);
    }

    #[test]
    fn default_registry_has_expected_count() {
        // 21 entries in the static ICONS_TS body — keep the count
        // pinned so dropping or adding an entry is a deliberate state
        // schema change, not a silent drift.
        let cfg = IconConfig::default();
        assert_eq!(cfg.registry.len(), 21);
    }

    #[test]
    fn default_registry_has_required_navigation_entries() {
        let cfg = IconConfig::default();
        for k in ["home", "dashboard", "settings", "cog", "user", "users", "tools"] {
            let key = IconKey::new(k).expect("test key");
            assert!(cfg.registry.contains_key(&key), "default registry missing {k}");
        }
    }

    #[test]
    fn default_registry_has_required_action_entries() {
        let cfg = IconConfig::default();
        for k in ["add", "edit", "delete", "save", "cancel", "back"] {
            let key = IconKey::new(k).expect("test key");
            assert!(cfg.registry.contains_key(&key), "default registry missing {k}");
        }
    }

    #[test]
    fn default_registry_class_strings_match_static() {
        // Sample-check a handful against the static ICONS_TS body to
        // catch typo drift in either side. The full string-by-string
        // parity test lives in Wave B's codegen lane.
        let cfg = IconConfig::default();
        let expected: &[(&str, &str)] = &[("home", "pi pi-home"), ("dashboard", "pi pi-th-large"), ("warning", "pi pi-exclamation-triangle"), ("refresh", "pi pi-refresh")];
        for (k, v) in expected {
            let key = IconKey::new(*k).expect("test key");
            let class = cfg.registry.get(&key).expect("present");
            assert_eq!(class.as_str(), *v);
        }
    }
}
