use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;
use serde::Serialize;
use serde_json::{to_value, Value};
use stylance::import_crate_style;

use crate::cata_log;
use crate::structs::services::render::{StatBuilder, StatField, StatFormatter};

import_crate_style!(style, "src/services/render/stat.module.scss");

const DEFAULT_EMPTY_TEXT: &str = "No stats.";

impl<T> StatBuilder<T>
where
    T: Serialize + Clone + Send + Sync + 'static,
{
    pub fn new(item: T) -> Self {
        Self {
            item,
            stats: Vec::new(),
            formatters: HashMap::new(),
            class_grid: None,
            class_card: None,
            class_label: None,
            class_value: None,
            empty_text: DEFAULT_EMPTY_TEXT.to_string(),
        }
    }

    pub fn stat(mut self, field: &str, label: &str) -> Self {
        self.stats.push(StatField {
            field: field.to_string(),
            label: label.to_string(),
        });
        self
    }

    pub fn formatter<F>(mut self, field: &str, f: F) -> Self
    where
        F: Fn(&Value) -> AnyView + Send + Sync + 'static,
    {
        self.formatters.insert(field.to_string(), Arc::new(f) as StatFormatter);
        self
    }

    pub fn class_grid(mut self, c: &str) -> Self {
        self.class_grid = Some(c.to_string());
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
        if self.stats.is_empty() {
            return render_empty(&self.empty_text);
        }

        let serialized = match to_value(&self.item) {
            Ok(v) => v,
            Err(err) => {
                cata_log!(Error, format!("StatBuilder serialize failure: {}", err));
                return render_empty(&format!("render error: {}", err));
            }
        };

        let map = match serialized {
            Value::Object(m) => m,
            other => {
                cata_log!(Error, format!("StatBuilder<T>: T must serialize to JSON object, got {:?}", other));
                return render_empty(&self.empty_text);
            }
        };

        let grid_class = merge_class(style::grid, self.class_grid.as_deref());
        let card_class = merge_class(style::card, self.class_card.as_deref());
        let label_class = merge_class(style::label, self.class_label.as_deref());
        let value_class = merge_class(style::value, self.class_value.as_deref());

        let formatters = self.formatters.clone();

        let cards: Vec<AnyView> = self
            .stats
            .into_iter()
            .map(|stat| render_stat_card(stat, &map, &formatters, &card_class, &label_class, &value_class))
            .collect();

        view! {
            <section class=grid_class>{cards}</section>
        }
        .into_any()
    }
}

fn render_stat_card(stat: StatField, map: &serde_json::Map<String, Value>, formatters: &HashMap<String, StatFormatter>, card_class: &str, label_class: &str, value_class: &str) -> AnyView {
    let value_view = match map.get(&stat.field) {
        Some(v) => match formatters.get(&stat.field) {
            Some(f) => f(v),
            None => fallback_value(v),
        },
        None => fallback_value(&Value::Null),
    };
    let card_cls = card_class.to_string();
    let label_cls = label_class.to_string();
    let value_cls = value_class.to_string();
    let label_text = stat.label;
    view! {
        <article class=card_cls>
            <span class=label_cls>{label_text}</span>
            <span class=value_cls>{value_view}</span>
        </article>
    }
    .into_any()
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

    #[derive(Serialize, Clone)]
    struct Metrics {
        users: i64,
        revenue: f64,
        churn: f64,
    }

    fn sample() -> Metrics {
        Metrics {
            users: 1234,
            revenue: 9876.5,
            churn: 0.04,
        }
    }

    fn render_to_string<T>(builder: StatBuilder<T>) -> String
    where
        T: Serialize + Clone + Send + Sync + 'static,
    {
        builder.into_view().to_html()
    }

    #[test]
    fn empty_when_no_stats_declared() {
        let html = render_to_string(StatBuilder::new(sample()).empty_text("nothing here"));
        assert!(html.contains("nothing here"), "expected empty fallback in: {}", html);
        assert!(!html.contains("<section"), "expected no <section> in fallback: {}", html);
    }

    #[test]
    fn renders_grid_with_cards() {
        let html = render_to_string(StatBuilder::new(sample()).stat("users", "Users").stat("revenue", "Revenue"));
        assert!(html.contains("<section"), "expected <section> in: {}", html);
        assert!(html.contains("<article"), "expected <article> in: {}", html);
        assert!(html.contains("Users"), "expected Users label: {}", html);
        assert!(html.contains("1234"), "expected users count: {}", html);
    }

    #[test]
    fn formatter_override_used() {
        let html = render_to_string(StatBuilder::new(sample()).stat("users", "Users").formatter("users", |v| {
            let n = match v {
                Value::Number(n) => n.as_i64().unwrap_or(0),
                _ => 0,
            };
            let text = format!("{}+", n);
            view! { <strong class="custom-fmt">{text}</strong> }.into_any()
        }));
        assert!(html.contains("custom-fmt"), "formatter class missing: {}", html);
        assert!(html.contains("<strong"), "formatter tag missing: {}", html);
        assert!(html.contains("1234+"), "formatter content missing: {}", html);
    }

    #[test]
    fn class_injection_threads_through() {
        let html = render_to_string(
            StatBuilder::new(sample())
                .stat("users", "Users")
                .class_grid("my-grid")
                .class_card("my-card")
                .class_label("my-label")
                .class_value("my-value"),
        );
        assert!(html.contains("my-grid"), "grid class missing: {}", html);
        assert!(html.contains("my-card"), "card class missing: {}", html);
        assert!(html.contains("my-label"), "label class missing: {}", html);
        assert!(html.contains("my-value"), "value class missing: {}", html);
    }
}
