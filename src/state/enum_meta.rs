use std::collections::BTreeMap;


use serde::{Deserialize, Serialize};

pub const ENUM_META_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumMeta {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub variants: BTreeMap<String, VariantMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantMeta {
    pub label: String,
    #[serde(default)]
    pub category: Option<String>,
}

impl EnumMeta {
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            schema_version: ENUM_META_SCHEMA_VERSION,
            name: name.into(),
            variants: BTreeMap::new(),
        }
    }

    pub fn lookup<'a>(&'a self, sql_variant: &str) -> Option<&'a VariantMeta> {
        self.variants.get(sql_variant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ron_roundtrip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let cfg = ron::ser::PrettyConfig::new().depth_limit(64).indentor("  ".to_string()).struct_names(true);
        let s = ron::ser::to_string_pretty(value, cfg).unwrap_or_else(|e| panic!("serialize failed: {e}\nvalue: {value:?}"));
        ron::from_str::<T>(&s).unwrap_or_else(|e| panic!("deserialize failed: {e}\nRON:\n{s}"))
    }

    #[test]
    fn empty_meta_round_trips() {
        let m = EnumMeta::empty("user_role");
        let after = ron_roundtrip(&m);
        assert_eq!(after, m);
    }

    #[test]
    fn populated_meta_round_trips() {
        let mut m = EnumMeta::empty("feature_kind");
        m.variants.insert(
            "abs".to_string(),
            VariantMeta {
                label: "ABS".to_string(),
                category: Some("Siguranță".to_string()),
            },
        );
        m.variants.insert(
            "esp".to_string(),
            VariantMeta {
                label: "ESP".to_string(),
                category: Some("Siguranță".to_string()),
            },
        );
        m.variants.insert(
            "alta".to_string(),
            VariantMeta {
                label: "Altă culoare".to_string(),
                category: None,
            },
        );
        let after = ron_roundtrip(&m);
        assert_eq!(after, m);
    }

    #[test]
    fn lookup_returns_none_for_missing() {
        let m = EnumMeta::empty("color_kind");
        assert!(m.lookup("rosu").is_none());
    }

    #[test]
    fn lookup_returns_some_for_present() {
        let mut m = EnumMeta::empty("color_kind");
        m.variants.insert(
            "rosu".to_string(),
            VariantMeta {
                label: "Roșu".to_string(),
                category: None,
            },
        );
        assert_eq!(m.lookup("rosu").map(|v| v.label.as_str()), Some("Roșu"));
    }
}
