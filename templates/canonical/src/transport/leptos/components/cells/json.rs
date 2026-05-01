use leptos::prelude::*;
use serde_json::Value;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/json.module.scss");

fn pretty_json(value: &Value) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(s) => s,
        Err(e) => {
            crate::cata_log!(Error, format!("json_cell: pretty_json failed: {}", e));
            format!("{}", e)
        }
    }
}

fn collapsed_summary(value: &Value) -> String {
    match value {
        Value::Object(m) => format!("{{\u{2026}}} ({} keys)", m.len()),
        Value::Array(a) => format!("[\u{2026}] ({} items)", a.len()),
        Value::String(s) => s.chars().take(32).collect(),
        Value::Number(n) => format!("{}", n),
        Value::Bool(b) => format!("{}", b),
        Value::Null => "null".to_string(),
    }
}

#[component]
pub fn JsonCell(
    value: Value,
    #[prop(default = false)] collapsed: bool,
) -> impl IntoView {
    let pretty = pretty_json(&value);
    if collapsed {
        let summary = collapsed_summary(&value);
        view! {
            <details class=style::details>
                <summary class=style::summary>{summary}</summary>
                <pre class=style::pre>{pretty}</pre>
            </details>
        }
        .into_any()
    } else {
        view! {
            <pre class=style::pre>{pretty}</pre>
        }
        .into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pretty_format() {
        let val = json!({"a": 1});
        let s = pretty_json(&val);
        assert!(s.contains("\"a\""));
        assert!(s.contains("1"));
    }

    #[test]
    fn summary_objects() {
        let val = json!({"x": 1, "y": 2});
        assert_eq!(collapsed_summary(&val), "{\u{2026}} (2 keys)");
    }

    #[test]
    fn summary_arrays() {
        let val = json!([1, 2, 3]);
        assert_eq!(collapsed_summary(&val), "[\u{2026}] (3 items)");
    }
}
