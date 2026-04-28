use std::path::Path;

use crate::{
    governor::{
        rules::{
            helpers::{extension_is, path_contains, rel_path_str, snippet_of},
            traits::FileRule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

const PRIMEVUE_PRIMITIVES: &[&str] = &[
    "Button",
    "Card",
    "Dialog",
    "Drawer",
    "Modal",
    "Dropdown",
    "Select",
    "Checkbox",
    "RadioButton",
    "Slider",
    "ProgressBar",
    "Sidebar",
    "Toolbar",
    "Breadcrumb",
    "Paginator",
    "Skeleton",
    "Toast",
    "Tabs",
    "TabView",
    "Tab",
    "DataTable",
    "Tree",
    "TreeTable",
    "Calendar",
    "DatePicker",
    "Tooltip",
];

pub struct PrimeVueReinvented;

impl PrimeVueReinvented {
    pub fn new() -> Self {
        Self
    }
}

fn component_name_from_path(file: &Path) -> Option<String> {
    let path = rel_path_str(file);
    let last = path.rsplit('/').next()?;
    let stem = last.strip_suffix(".vue")?;
    Some(stem.to_string())
}

fn is_user_components(file: &Path) -> bool {
    !path_contains(file, "/generated/")
}

impl FileRule for PrimeVueReinvented {
    fn name(&self) -> &'static str {
        "PrimeVueReinvented"
    }

    fn check_file(&self, file: &Path, _contents: &str, _config: &FeLintState) -> Vec<Violation> {
        if !extension_is(file, "vue") {
            return Vec::new();
        }
        if !is_user_components(file) {
            return Vec::new();
        }
        let name = match component_name_from_path(file) {
            Some(n) => n,
            None => return Vec::new(),
        };
        if !PRIMEVUE_PRIMITIVES.iter().any(|p| *p == name) {
            return Vec::new();
        }
        vec![Violation::new(
            "PrimeVueReinvented",
            file.to_path_buf(),
            1,
            snippet_of(&format!("custom component named {name} shadows PrimeVue primitive")),
            "use PrimeVue's component directly; name domain wrappers after the domain (OrderActionsMenu, not Menu)",
        )]
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn run(file: &str) -> Vec<Violation> {
        let rule = PrimeVueReinvented::new();
        let cfg = FeLintState::default();
        rule.check_file(&PathBuf::from(file), "", &cfg)
    }

    #[test]
    fn flags_custom_button_component() {
        let v = run("frontend/src/components/Button.vue");
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn flags_custom_dialog_component() {
        let v = run("frontend/src/components/Dialog.vue");
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn allows_domain_wrapper_name() {
        let v = run("frontend/src/components/OrderActionsMenu.vue");
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_outside_custom_components() {
        let v = run("frontend/src/generated/components/Button.vue");
        assert!(v.is_empty());
    }
}
