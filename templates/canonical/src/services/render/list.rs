use std::sync::Arc;

use leptos::prelude::*;
use serde::Serialize;
use serde_json::{to_value, Value};
use stylance::import_crate_style;

use crate::cata_log;
use crate::structs::services::render::{ListBuilder, ListItemTemplate, ListType};

import_crate_style!(style, "src/services/render/list.module.scss");

const DEFAULT_EMPTY_TEXT: &str = "No items.";

impl<T> ListBuilder<T>
where
    T: Serialize + Clone + Send + Sync + 'static,
{
    pub fn new(list_type: ListType, items: Vec<T>) -> Self {
        Self {
            list_type,
            items,
            ignore: Vec::new(),
            item_template: None,
            class_list: None,
            class_item: None,
            empty_text: DEFAULT_EMPTY_TEXT.to_string(),
        }
    }

    pub fn ignore(mut self, fields: &str) -> Self {
        self.ignore = fields
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        self
    }

    pub fn item_template<F>(mut self, f: F) -> Self
    where
        F: Fn(&Value) -> AnyView + Send + Sync + 'static,
    {
        self.item_template = Some(Arc::new(f) as ListItemTemplate);
        self
    }

    pub fn class_list(mut self, c: &str) -> Self {
        self.class_list = Some(c.to_string());
        self
    }

    pub fn class_item(mut self, c: &str) -> Self {
        self.class_item = Some(c.to_string());
        self
    }

    pub fn empty_text(mut self, msg: &str) -> Self {
        self.empty_text = msg.to_string();
        self
    }

    pub fn into_view(self) -> AnyView {
        if self.items.is_empty() {
            return render_empty(&self.empty_text, merge_class(style::empty, None));
        }

        let serialized: Result<Vec<Value>, serde_json::Error> = self.items.iter().map(to_value).collect();
        let rows = match serialized {
            Ok(rs) => rs,
            Err(err) => {
                cata_log!(Error, format!("ListBuilder serialize failure: {}", err));
                return render_empty(&format!("render error: {}", err), merge_class(style::empty, None));
            }
        };

        let list_class = merge_class(style::list, self.class_list.as_deref());
        let item_class = merge_class(style::item, self.class_item.as_deref());

        let template = self.item_template.clone();
        let ignore = self.ignore.clone();

        let item_views: Vec<AnyView> = rows
            .into_iter()
            .map(|row| render_item(row, item_class.clone(), template.clone(), &ignore))
            .collect();

        match self.list_type {
            ListType::Unordered => view! {
                <ul class=list_class>{item_views}</ul>
            }
            .into_any(),
            ListType::Ordered => view! {
                <ol class=list_class>{item_views}</ol>
            }
            .into_any(),
        }
    }
}

fn render_item(row: Value, item_class: String, template: Option<ListItemTemplate>, ignore: &[String]) -> AnyView {
    let inner = match template {
        Some(t) => t(&row),
        None => default_item_view(&row, ignore),
    };
    view! {
        <li class=item_class>{inner}</li>
    }
    .into_any()
}

fn default_item_view(row: &Value, ignore: &[String]) -> AnyView {
    let text = match row {
        Value::Object(map) => map
            .iter()
            .filter(|(k, _)| !ignore.iter().any(|i| i == *k))
            .map(|(k, v)| format!("{}: {}", k, value_to_string(v)))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Null => String::new(),
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => value_to_string(row),
    };
    view! { <span>{text}</span> }.into_any()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => format!("{}", value),
    }
}

fn render_empty(message: &str, empty_class: String) -> AnyView {
    let msg = message.to_string();
    view! { <p class=empty_class>{msg}</p> }.into_any()
}

fn merge_class(base: &str, extra: Option<&str>) -> String {
    match extra {
        Some(c) => {
            if c.is_empty() {
                base.to_string()
            } else {
                format!("{} {}", base, c)
            }
        }
        None => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use leptos::prelude::*;
    use serde::Serialize;

    use super::*;

    #[derive(Serialize, Clone)]
    struct DemoItem {
        id: i64,
        name: String,
        secret: String,
    }

    fn demo_items() -> Vec<DemoItem> {
        vec![
            DemoItem {
                id: 1,
                name: "alpha".to_string(),
                secret: "hush".to_string(),
            },
            DemoItem {
                id: 2,
                name: "beta".to_string(),
                secret: "shh".to_string(),
            },
        ]
    }

    fn render_to_string<T>(builder: ListBuilder<T>) -> String
    where
        T: Serialize + Clone + Send + Sync + 'static,
    {
        builder.into_view().to_html()
    }

    #[test]
    fn empty_vec_renders_fallback() {
        let html = render_to_string(ListBuilder::<DemoItem>::new(ListType::Unordered, Vec::new()));
        assert!(html.contains("No items."), "expected default empty text in: {}", html);
        assert!(!html.contains("<ul"), "expected no <ul> in fallback: {}", html);
    }

    #[test]
    fn empty_text_override() {
        let html = render_to_string(ListBuilder::<DemoItem>::new(ListType::Unordered, Vec::new()).empty_text("nothing here"));
        assert!(html.contains("nothing here"), "expected override empty text in: {}", html);
    }

    #[test]
    fn unordered_renders_ul() {
        let html = render_to_string(ListBuilder::new(ListType::Unordered, demo_items()));
        assert!(html.contains("<ul"), "expected <ul> in: {}", html);
        assert!(!html.contains("<ol"), "did not expect <ol> in: {}", html);
    }

    #[test]
    fn ordered_renders_ol() {
        let html = render_to_string(ListBuilder::new(ListType::Ordered, demo_items()));
        assert!(html.contains("<ol"), "expected <ol> in: {}", html);
    }

    #[test]
    fn ignored_field_absent_from_default_render() {
        let html = render_to_string(ListBuilder::new(ListType::Unordered, demo_items()).ignore("secret"));
        assert!(!html.contains("hush"), "secret value leaked: {}", html);
        assert!(!html.contains("shh"), "secret value leaked: {}", html);
        assert!(html.contains("alpha"), "expected name to remain: {}", html);
    }

    #[test]
    fn ignore_accepts_comma_and_whitespace_separators() {
        let html = render_to_string(ListBuilder::new(ListType::Unordered, demo_items()).ignore("secret, id"));
        assert!(!html.contains("hush"), "secret should be ignored: {}", html);
        assert!(!html.contains("id:"), "id should be ignored: {}", html);
        assert!(html.contains("alpha"), "name should remain: {}", html);
    }

    #[test]
    fn item_template_override_used() {
        let html = render_to_string(ListBuilder::new(ListType::Unordered, demo_items()).item_template(|v| {
            let raw = match v {
                Value::Object(m) => match m.get("name") {
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                },
                _ => String::new(),
            };
            view! { <strong class="custom-fmt">{raw}</strong> }.into_any()
        }));
        assert!(html.contains("custom-fmt"), "template class missing in: {}", html);
        assert!(html.contains("<strong"), "template tag missing in: {}", html);
        assert!(html.contains("alpha"), "template content missing in: {}", html);
    }

    #[test]
    fn class_injection_threads_through() {
        let html = render_to_string(
            ListBuilder::new(ListType::Unordered, demo_items())
                .class_list("my-list")
                .class_item("my-item"),
        );
        assert!(html.contains("my-list"), "list class missing: {}", html);
        assert!(html.contains("my-item"), "item class missing: {}", html);
    }
}
