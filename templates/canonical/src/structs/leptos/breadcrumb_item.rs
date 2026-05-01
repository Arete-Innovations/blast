use crate::structs::leptos::route_name::RouteName;

#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub label: String,
    pub to: Option<RouteName>,
}

impl BreadcrumbItem {
    pub fn linked(label: impl Into<String>, to: RouteName) -> Self {
        Self {
            label: label.into(),
            to: Some(to),
        }
    }

    pub fn current(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            to: None,
        }
    }
}
