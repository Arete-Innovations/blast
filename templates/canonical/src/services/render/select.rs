use leptos::prelude::*;
use serde::Serialize;
use serde_json::{to_value, Value};
use stylance::import_crate_style;

use crate::cata_log;
use crate::structs::services::render::SelectBuilder;

import_crate_style!(style, "src/services/render/select.module.scss");

const DEFAULT_EMPTY_TEXT: &str = "No options.";

impl<T> SelectBuilder<T>
where
    T: Serialize + Clone + Send + Sync + 'static,
{
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            label_field: None,
            value_field: None,
            name: None,
            placeholder: None,
            class_select: None,
            class_option: None,
            empty_text: DEFAULT_EMPTY_TEXT.to_string(),
        }
    }

    pub fn label_field(mut self, name: &str) -> Self {
        self.label_field = Some(name.to_string());
        self
    }

    pub fn value_field(mut self, name: &str) -> Self {
        self.value_field = Some(name.to_string());
        self
    }

    pub fn name(mut self, form_name: &str) -> Self {
        self.name = Some(form_name.to_string());
        self
    }

    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = Some(p.to_string());
        self
    }

    pub fn class_select(mut self, c: &str) -> Self {
        self.class_select = Some(c.to_string());
        self
    }

    pub fn class_option(mut self, c: &str) -> Self {
        self.class_option = Some(c.to_string());
        self
    }

    pub fn empty_text(mut self, msg: &str) -> Self {
        self.empty_text = msg.to_string();
        self
    }

    pub fn into_view(self) -> AnyView {
        if self.items.is_empty() {
            return render_empty(&self.empty_text);
        }

        let serialized: Result<Vec<Value>, serde_json::Error> = self.items.iter().map(to_value).collect();
        let rows = match serialized {
            Ok(rs) => rs,
            Err(err) => {
                cata_log!(Error, format!("SelectBuilder serialize failure: {}", err));
                return render_empty(&format!("render error: {}", err));
            }
        };

        let select_class = merge_class(style::select, self.class_select.as_deref());
        let option_class = merge_class(style::option, self.class_option.as_deref());
        let label_field = self.label_field.clone();
        let value_field = self.value_field.clone();

        let mut option_views: Vec<AnyView> = Vec::new();

        match self.placeholder.clone() {
            Some(p) => option_views.push(render_placeholder(&p, &option_class)),
            None => {}
        }

        for row in rows.into_iter() {
            option_views.push(render_option(row, label_field.as_deref(), value_field.as_deref(), &option_class));
        }

        let name_attr = match self.name.clone() {
            Some(n) => n,
            None => String::from(""),
        };

        view! {
            <select class=select_class name=name_attr>{option_views}</select>
        }
        .into_any()
    }
}

fn render_option(row: Value, label_field: Option<&str>, value_field: Option<&str>, option_class: &str) -> AnyView {
    let map = match row {
        Value::Object(m) => m,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => serde_json::Map::new(),
    };

    let value = match value_field.and_then(|f| map.get(f)) {
        Some(v) => value_to_string(v),
        None => first_value_string(&map),
    };

    let label = match label_field.and_then(|f| map.get(f)) {
        Some(v) => value_to_string(v),
        None => value.clone(),
    };

    let cls = option_class.to_string();
    view! {
        <option class=cls value=value>{label}</option>
    }
    .into_any()
}

fn render_placeholder(text: &str, option_class: &str) -> AnyView {
    let label = text.to_string();
    let cls = option_class.to_string();
    view! {
        <option class=cls value="" disabled=true selected=true>{label}</option>
    }
    .into_any()
}

fn first_value_string(map: &serde_json::Map<String, Value>) -> String {
    match map.values().next() {
        Some(v) => value_to_string(v),
        None => value_to_string(&Value::Null),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::from(""),
        Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => format!("{}", value),
    }
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
    struct Option1 {
        id: i64,
        label: String,
    }

    fn options() -> Vec<Option1> {
        vec![
            Option1 {
                id: 1,
                label: "alpha".to_string(),
            },
            Option1 {
                id: 2,
                label: "beta".to_string(),
            },
        ]
    }

    fn render_to_string<T>(builder: SelectBuilder<T>) -> String
    where
        T: Serialize + Clone + Send + Sync + 'static,
    {
        builder.into_view().to_html()
    }

    #[test]
    fn empty_vec_renders_fallback() {
        let html = render_to_string(SelectBuilder::<Option1>::new(Vec::new()));
        assert!(html.contains("No options."), "expected default empty text in: {}", html);
        assert!(!html.contains("<select"), "expected no <select> in fallback: {}", html);
    }

    #[test]
    fn label_value_fields_used() {
        let html = render_to_string(SelectBuilder::new(options()).label_field("label").value_field("id"));
        assert!(html.contains("<select"), "expected <select> in: {}", html);
        assert!(html.contains("alpha"), "expected label alpha in: {}", html);
        assert!(html.contains("value=\"1\""), "expected value=1 in: {}", html);
        assert!(html.contains("value=\"2\""), "expected value=2 in: {}", html);
    }

    #[test]
    fn name_attribute_emits() {
        let html = render_to_string(SelectBuilder::new(options()).name("category"));
        assert!(html.contains("name=\"category\""), "expected name=category in: {}", html);
    }

    #[test]
    fn placeholder_emits_disabled_option() {
        let html = render_to_string(SelectBuilder::new(options()).placeholder("Choose..."));
        assert!(html.contains("Choose..."), "expected placeholder text in: {}", html);
        assert!(html.contains("disabled"), "expected disabled attribute in: {}", html);
    }

    #[test]
    fn class_injection_threads_through() {
        let html = render_to_string(
            SelectBuilder::new(options())
                .class_select("my-select")
                .class_option("my-option"),
        );
        assert!(html.contains("my-select"), "select class missing: {}", html);
        assert!(html.contains("my-option"), "option class missing: {}", html);
    }
}
