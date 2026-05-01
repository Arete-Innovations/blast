use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;
use serde::Serialize;
use serde_json::{to_value, Value};
use stylance::import_crate_style;

use crate::cata_log;
use crate::structs::services::render::{DetailBuilder, DetailFormatter};

import_crate_style!(style, "src/services/render/detail.module.scss");

const DEFAULT_EMPTY_TEXT: &str = "No details.";

impl<T> DetailBuilder<T>
where
    T: Serialize + Clone + Send + Sync + 'static,
{
    pub fn new(item: T) -> Self {
        Self {
            item,
            ignore: Vec::new(),
            labels: HashMap::new(),
            formatters: HashMap::new(),
            class_card: None,
            class_label: None,
            class_value: None,
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

    pub fn label(mut self, field: &str, label: &str) -> Self {
        self.labels.insert(field.to_string(), label.to_string());
        self
    }

    pub fn formatter<F>(mut self, field: &str, f: F) -> Self
    where
        F: Fn(&Value) -> AnyView + Send + Sync + 'static,
    {
        self.formatters.insert(field.to_string(), Arc::new(f) as DetailFormatter);
        self
    }

    pub fn class_card(mut self, c: &str) -> Self {
        self.class_card = Some(c.to_string());
        self
    }

    pub fn class_label(mut self, c: &str) -> Self {
        self.class_label = Some(c.to_string());
        self
    }

    pub fn class_value(mut self, c: &str) -> Self {
        self.class_value = Some(c.to_string());
        self
    }

    pub fn empty_text(mut self, msg: &str) -> Self {
        self.empty_text = msg.to_string();
        self
    }

    pub fn into_view(self) -> AnyView {
        let serialized = match to_value(&self.item) {
            Ok(v) => v,
            Err(err) => {
                cata_log!(Error, format!("DetailBuilder serialize failure: {}", err));
                return render_empty(&format!("render error: {}", err));
            }
        };

        let map = match serialized {
            Value::Object(m) => m,
            other => {
                cata_log!(Error, format!("DetailBuilder<T>: T must serialize to JSON object, got {:?}", other));
                return render_empty(&self.empty_text);
            }
        };

        let entries: Vec<(String, Value)> = map.into_iter().filter(|(k, _)| !self.ignore.iter().any(|i| i == k)).collect();

        if entries.is_empty() {
            return render_empty(&self.empty_text);
        }

        let card_class = merge_class(style::card, self.class_card.as_deref());
        let label_class = merge_class(style::label, self.class_label.as_deref());
        let value_class = merge_class(style::value, self.class_value.as_deref());

        let labels = self.labels.clone();
        let formatters = self.formatters.clone();

        let pairs: Vec<AnyView> = entries
            .into_iter()
            .flat_map(|(field, value)| {
                let label_text = match labels.get(&field).cloned() {
                    Some(l) => l,
                    None => field.clone(),
                };
                let value_view = match formatters.get(&field) {
                    Some(f) => f(&value),
                    None => fallback_value(&value),
                };
                let label_class_clone = label_class.clone();
                let value_class_clone = value_class.clone();
                let dt = view! { <dt class=label_class_clone>{label_text}</dt> }.into_any();
                let dd = view! { <dd class=value_class_clone>{value_view}</dd> }.into_any();
                vec![dt, dd]
            })
            .collect();

        view! {
            <dl class=card_class>{pairs}</dl>
        }
        .into_any()
    }
}

fn fallback_value(value: &Value) -> AnyView {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => format!("{}", value),
    };
    view! { <span>{text}</span> }.into_any()
}

fn render_empty(message: &str) -> AnyView {
    let msg = message.to_string();
    let cls = style::empty.to_string();
    view! { <p class=cls>{msg}</p> }.into_any()
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

    #[derive(Serialize, Clone, Default)]
    struct Profile {
        id: i64,
        name: String,
        secret: String,
    }

    fn sample() -> Profile {
        Profile {
            id: 7,
            name: "Alice".to_string(),
            secret: "hush".to_string(),
        }
    }

    fn render_to_string<T>(builder: DetailBuilder<T>) -> String
    where
        T: Serialize + Clone + Send + Sync + 'static,
    {
        builder.into_view().to_html()
    }

    #[test]
    fn empty_when_all_ignored_uses_fallback() {
        let html = render_to_string(DetailBuilder::new(sample()).ignore("id, name, secret").empty_text("nothing here"));
        assert!(html.contains("nothing here"), "expected empty fallback in: {}", html);
        assert!(!html.contains("<dl"), "expected no <dl> in fallback: {}", html);
    }

    #[test]
    fn renders_dl_with_pairs() {
        let html = render_to_string(DetailBuilder::new(sample()));
        assert!(html.contains("<dl"), "expected <dl> in: {}", html);
        assert!(html.contains("<dt"), "expected <dt> in: {}", html);
        assert!(html.contains("<dd"), "expected <dd> in: {}", html);
        assert!(html.contains("Alice"), "expected name value: {}", html);
    }

    #[test]
    fn ignored_field_excluded() {
        let html = render_to_string(DetailBuilder::new(sample()).ignore("secret"));
        assert!(!html.contains("hush"), "secret leaked: {}", html);
        assert!(html.contains("Alice"), "expected name to remain: {}", html);
    }

    #[test]
    fn label_override_replaces_default() {
        let html = render_to_string(DetailBuilder::new(sample()).label("name", "Display Name"));
        assert!(html.contains("Display Name"), "expected label override: {}", html);
    }

    #[test]
    fn formatter_override_used() {
        let html = render_to_string(DetailBuilder::new(sample()).formatter("name", |v| {
            let text = match v {
                Value::String(s) => s.clone(),
                other => format!("{}", other),
            };
            view! { <strong class="custom-fmt">{text}</strong> }.into_any()
        }));
        assert!(html.contains("custom-fmt"), "formatter class missing: {}", html);
        assert!(html.contains("<strong"), "formatter tag missing: {}", html);
    }

    #[test]
    fn class_injection_threads_through() {
        let html = render_to_string(
            DetailBuilder::new(sample())
                .class_card("my-card")
                .class_label("my-label")
                .class_value("my-value"),
        );
        assert!(html.contains("my-card"), "card class missing: {}", html);
        assert!(html.contains("my-label"), "label class missing: {}", html);
        assert!(html.contains("my-value"), "value class missing: {}", html);
    }
}
