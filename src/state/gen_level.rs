use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GenLevel {
    Struct,
    Model,
    Route,
    Types,
    Composables,
    Components,
    Pages,
}

impl Default for GenLevel {
    fn default() -> Self {
        Self::Composables
    }
}

impl GenLevel {
    pub const ALL: &'static [GenLevel] = &[GenLevel::Struct, GenLevel::Model, GenLevel::Route, GenLevel::Types, GenLevel::Composables, GenLevel::Components, GenLevel::Pages];

    pub fn label(self) -> &'static str {
        match self {
            GenLevel::Struct => "Struct",
            GenLevel::Model => "Model",
            GenLevel::Route => "Route",
            GenLevel::Types => "Types",
            GenLevel::Composables => "Composables",
            GenLevel::Components => "Components",
            GenLevel::Pages => "Pages",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            GenLevel::Struct => "Struct: structs/generated/<r>.rs only (data shape)",
            GenLevel::Model => "Model: + models/generated/<r>.rs (Diesel CRUD)",
            GenLevel::Route => "Route: + flows + http_api (REST + WS)",
            GenLevel::Types => "Types: + leptos isomorphic data helpers (load_*, do_*) skeleton",
            GenLevel::Composables => "Composables: + validators (default)",
            GenLevel::Components => "Components: + leptos forms (CreateForm/EditForm with thaw inputs)",
            GenLevel::Pages => "Pages: + leptos pages (List/Detail/Create/Edit) wrapped in AuthGuard + PageShell",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_composables() {
        assert_eq!(GenLevel::default(), GenLevel::Composables);
    }

    #[test]
    fn ordering_is_monotonic() {
        assert!(GenLevel::Struct < GenLevel::Model);
        assert!(GenLevel::Model < GenLevel::Route);
        assert!(GenLevel::Route < GenLevel::Types);
        assert!(GenLevel::Types < GenLevel::Composables);
        assert!(GenLevel::Composables < GenLevel::Components);
        assert!(GenLevel::Components < GenLevel::Pages);
    }

    #[test]
    fn implies_lower_levels() {
        assert!(GenLevel::Pages >= GenLevel::Struct);
        assert!(GenLevel::Composables >= GenLevel::Types);
        assert!(GenLevel::Composables >= GenLevel::Route);
        assert!(GenLevel::Composables >= GenLevel::Model);
        assert!(GenLevel::Composables >= GenLevel::Struct);
    }

    #[test]
    fn all_lists_seven_levels() {
        assert_eq!(GenLevel::ALL.len(), 7);
    }

    #[test]
    fn ron_roundtrip() {
        for level in GenLevel::ALL {
            let ser = ron::to_string(level).expect("serialize");
            let de: GenLevel = ron::from_str(&ser).expect("deserialize");
            assert_eq!(*level, de);
        }
    }
}
