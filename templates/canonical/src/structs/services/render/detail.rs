use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::AnyView;
use serde_json::Value;

pub type DetailFormatter = Arc<dyn Fn(&Value) -> AnyView + Send + Sync + 'static>;

pub struct DetailBuilder<T> {
    pub item: T,
    pub ignore: Vec<String>,
    pub labels: HashMap<String, String>,
    pub formatters: HashMap<String, DetailFormatter>,
    pub class_card: Option<String>,
    pub class_label: Option<String>,
    pub class_value: Option<String>,
    pub empty_text: String,
}
