use std::sync::Arc;

use leptos::prelude::AnyView;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListType {
    Unordered,
    Ordered,
}

pub type ListItemTemplate = Arc<dyn Fn(&Value) -> AnyView + Send + Sync + 'static>;

pub struct ListBuilder<T> {
    pub list_type: ListType,
    pub items: Vec<T>,
    pub ignore: Vec<String>,
    pub item_template: Option<ListItemTemplate>,
    pub class_list: Option<String>,
    pub class_item: Option<String>,
    pub empty_text: String,
}
