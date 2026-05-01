use std::collections::HashMap;

use leptos::prelude::*;
use serde::Serialize;
use serde_json::{to_value, Value};
use stylance::import_crate_style;

use crate::cata_log;
use crate::structs::services::render::{Formatter, TableBuilder, TableRenderClasses};

import_crate_style!(style, "src/services/render/table.module.scss");

const DEFAULT_EMPTY_TEXT: &str = "No items.";

impl<T> TableBuilder<T>
where
    T: Serialize + Clone + Send + Sync + 'static,
{
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            ignore: Vec::new(),
            table_class: None,
            tr_class: None,
            thead_class: None,
            td_class: None,
            formatters: HashMap::new(),
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

    pub fn class_table(mut self, c: &str) -> Self {
        self.table_class = Some(c.to_string());
        self
    }

    pub fn class_thead(mut self, c: &str) -> Self {
        self.thead_class = Some(c.to_string());
        self
    }

    pub fn class_tr(mut self, c: &str) -> Self {
        self.tr_class = Some(c.to_string());
        self
    }

    pub fn class_td(mut self, c: &str) -> Self {
        self.td_class = Some(c.to_string());
        self
    }

    pub fn formatter<F>(mut self, col: &str, f: F) -> Self
    where
        F: Fn(&Value) -> AnyView + Send + Sync + 'static,
    {
        self.formatters.insert(col.to_string(), Box::new(f) as Formatter);
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
                cata_log!(Error, format!("TableBuilder serialize failure: {}", err));
                return render_empty(&format!("render error: {}", err), merge_class(style::empty, None));
            }
        };

        let columns = extract_columns(&rows[0], &self.ignore);
        if columns.is_empty() {
            return render_empty(&self.empty_text, merge_class(style::empty, None));
        }

        render_table(rows, columns, self.formatters, TableRenderClasses {
            table: merge_class(style::table, self.table_class.as_deref()),
            thead: merge_class(style::thead, self.thead_class.as_deref()),
            tr: merge_class(style::tr, self.tr_class.as_deref()),
            td: merge_class(style::td, self.td_class.as_deref()),
        })
    }
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

fn extract_columns(first: &Value, ignore: &[String]) -> Vec<String> {
    match first {
        Value::Object(map) => map.keys().filter(|k| !ignore.iter().any(|i| i == *k)).cloned().collect(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => Vec::new(),
    }
}

fn render_empty(message: &str, empty_class: String) -> AnyView {
    let msg = message.to_string();
    view! { <p class=empty_class>{msg}</p> }.into_any()
}

fn render_table(rows: Vec<Value>, columns: Vec<String>, formatters: HashMap<String, Formatter>, classes: TableRenderClasses) -> AnyView {
    let header_cells: Vec<AnyView> = columns
        .iter()
        .map(|col| {
            let label = col.clone();
            view! { <th>{label}</th> }.into_any()
        })
        .collect();

    let body_rows: Vec<AnyView> = rows.into_iter().map(|row| render_row(row, &columns, &formatters, &classes.tr, &classes.td)).collect();

    view! {
        <table class=classes.table>
            <thead class=classes.thead>
                <tr>{header_cells}</tr>
            </thead>
            <tbody>{body_rows}</tbody>
        </table>
    }
    .into_any()
}

fn render_row(row: Value, columns: &[String], formatters: &HashMap<String, Formatter>, tr_class: &str, td_class: &str) -> AnyView {
    let map = match row {
        Value::Object(m) => m,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => serde_json::Map::new(),
    };

    let cells: Vec<AnyView> = columns.iter().map(|col| render_cell(col, map.get(col), formatters, td_class)).collect();

    let tr_class_owned = tr_class.to_string();
    view! {
        <tr class=tr_class_owned>
            {cells}
        </tr>
    }
    .into_any()
}

fn render_cell(col: &str, value: Option<&Value>, formatters: &HashMap<String, Formatter>, td_class: &str) -> AnyView {
    let td_class_owned = td_class.to_string();
    let inner = match value {
        Some(v) => match formatters.get(col) {
            Some(f) => f(v),
            None => fallback_cell(v),
        },
        None => view! { <span></span> }.into_any(),
    };

    view! {
        <td class=td_class_owned>{inner}</td>
    }
    .into_any()
}

fn fallback_cell(value: &Value) -> AnyView {
    let text = match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => format!("{}", value),
    };
    view! { <span>{text}</span> }.into_any()
}

#[cfg(test)]
mod tests {
    use leptos::prelude::*;
    use serde::Serialize;

    use super::*;

    #[derive(Serialize, Clone)]
    struct DemoRow {
        id: i64,
        name: String,
        secret: String,
    }

    fn demo_rows() -> Vec<DemoRow> {
        vec![
            DemoRow {
                id: 1,
                name: "alpha".to_string(),
                secret: "hush".to_string(),
            },
            DemoRow {
                id: 2,
                name: "beta".to_string(),
                secret: "shh".to_string(),
            },
        ]
    }

    fn render_to_string<T>(builder: TableBuilder<T>) -> String
    where
        T: Serialize + Clone + Send + Sync + 'static,
    {
        builder.into_view().to_html()
    }

    #[test]
    fn empty_vec_renders_fallback() {
        let html = render_to_string(TableBuilder::<DemoRow>::new(Vec::new()).empty_text("nothing here"));
        assert!(html.contains("nothing here"), "expected empty fallback text in: {}", html);
        assert!(!html.contains("<table"), "expected no <table> in fallback: {}", html);
    }

    #[test]
    fn empty_vec_uses_default_text() {
        let html = render_to_string(TableBuilder::<DemoRow>::new(Vec::new()));
        assert!(html.contains("No items."), "expected default empty text in: {}", html);
    }

    #[test]
    fn ignored_field_is_absent_from_headers_and_cells() {
        let html = render_to_string(TableBuilder::new(demo_rows()).ignore("secret"));
        assert!(html.contains("<table"), "expected table tag in: {}", html);
        assert!(html.contains(">id<"), "expected id header: {}", html);
        assert!(html.contains(">name<"), "expected name header: {}", html);
        assert!(!html.contains("hush"), "secret value leaked: {}", html);
        assert!(!html.contains("shh"), "secret value leaked: {}", html);
    }

    #[test]
    fn ignore_accepts_comma_and_whitespace_separators() {
        let html = render_to_string(TableBuilder::new(demo_rows()).ignore("secret, id"));
        assert!(!html.contains(">id<"), "id should be ignored: {}", html);
        assert!(!html.contains("hush"), "secret should be ignored: {}", html);
        assert!(html.contains(">name<"), "name should remain: {}", html);
    }

    #[test]
    fn formatter_override_used_for_column() {
        let html = render_to_string(TableBuilder::new(demo_rows()).formatter("name", |v| {
            let raw = match v {
                Value::String(s) => s.clone(),
                other => format!("{}", other),
            };
            view! { <strong class="custom-fmt">{raw}</strong> }.into_any()
        }));
        assert!(html.contains("custom-fmt"), "formatter class missing in: {}", html);
        assert!(html.contains("<strong"), "formatter tag missing in: {}", html);
        assert!(html.contains("alpha"), "formatter content missing in: {}", html);
    }

    #[test]
    fn class_injection_threads_through_to_html() {
        let html = render_to_string(
            TableBuilder::new(demo_rows())
                .class_table("my-table")
                .class_thead("my-thead")
                .class_tr("my-tr")
                .class_td("my-td"),
        );
        assert!(html.contains("my-table"), "table class missing: {}", html);
        assert!(html.contains("my-thead"), "thead class missing: {}", html);
        assert!(html.contains("my-tr"), "tr class missing: {}", html);
        assert!(html.contains("my-td"), "td class missing: {}", html);
    }

    #[test]
    fn rows_render_in_declared_order() {
        let html = render_to_string(TableBuilder::new(demo_rows()));
        let alpha_pos = match html.find("alpha") {
            Some(p) => p,
            None => panic!("alpha missing in: {}", html),
        };
        let beta_pos = match html.find("beta") {
            Some(p) => p,
            None => panic!("beta missing in: {}", html),
        };
        assert!(alpha_pos < beta_pos, "rows out of order: {}", html);
    }

    #[test]
    fn columns_follow_struct_field_order() {
        let html = render_to_string(TableBuilder::new(demo_rows()).ignore("secret"));
        let id_pos = match html.find(">id<") {
            Some(p) => p,
            None => panic!("id header missing: {}", html),
        };
        let name_pos = match html.find(">name<") {
            Some(p) => p,
            None => panic!("name header missing: {}", html),
        };
        assert!(id_pos < name_pos, "header order wrong: {}", html);
    }
}
