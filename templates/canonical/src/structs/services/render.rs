use std::collections::HashMap;

use leptos::prelude::AnyView;
use serde_json::Value;

pub type Formatter = Box<dyn Fn(&Value) -> AnyView + Send + Sync + 'static>;

pub struct TableBuilder<T> {
    pub items: Vec<T>,
    pub ignore: Vec<String>,
    pub table_class: Option<String>,
    pub thead_class: Option<String>,
    pub tr_class: Option<String>,
    pub td_class: Option<String>,
    pub formatters: HashMap<String, Formatter>,
    pub empty_text: String,
}

pub struct TableRenderClasses {
    pub table: String,
    pub thead: String,
    pub tr: String,
    pub td: String,
}
