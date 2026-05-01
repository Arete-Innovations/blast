use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::AnyView;
use serde_json::Value;

pub type StatFormatter = Arc<dyn Fn(&Value) -> AnyView + Send + Sync + 'static>;

#[derive(Clone)]
pub struct StatField {
    pub field: String,
    pub label: String,
}

pub struct StatBuilder<T> {
    pub item: T,
    pub stats: Vec<StatField>,
    pub formatters: HashMap<String, StatFormatter>,
    pub class_grid: Option<String>,
    pub class_card: Option<String>,
    pub class_label: Option<String>,
    pub class_value: Option<String>,
    pub empty_text: String,
}
