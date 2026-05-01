use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;

use leptos::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Number, Value};
use stylance::import_crate_style;

use crate::structs::services::render::{FieldMeta, FormBuilder, FormPlanEntry, InputKind};

import_crate_style!(style, "src/services/render/form.module.scss");

const PASSWORD_HIDE_SUFFIXES: &[&str] = &["password", "pwd", "_hash"];
const DATETIME_SUFFIXES: &[&str] = &["_at", "_on"];

impl<T> FormBuilder<T>
where
    T: Serialize + DeserializeOwned + Clone + Default + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            initial: None,
            fields: BTreeMap::new(),
            submit_label: String::from("Submit"),
            on_submit: None,
            class_form: String::new(),
            class_field: String::new(),
            class_submit: String::new(),
            _phantom: PhantomData,
        }
    }

    pub fn with_initial(initial: T) -> Self {
        let mut s = Self::new();
        s.initial = Some(initial);
        s
    }

    pub fn ignore(mut self, field: &str) -> Self {
        self.fields.entry(field.to_string()).or_default().ignored = true;
        self
    }

    pub fn label(mut self, field: &str, label: &str) -> Self {
        self.fields.entry(field.to_string()).or_default().label = Some(label.to_string());
        self
    }

    pub fn placeholder(mut self, field: &str, placeholder: &str) -> Self {
        self.fields.entry(field.to_string()).or_default().placeholder = Some(placeholder.to_string());
        self
    }

    pub fn input_kind(mut self, field: &str, kind: InputKind) -> Self {
        self.fields.entry(field.to_string()).or_default().kind = Some(kind);
        self
    }

    pub fn submit_label(mut self, label: &str) -> Self {
        self.submit_label = label.to_string();
        self
    }

    pub fn on_submit<F>(mut self, f: F) -> Self
    where
        F: Fn(T) + Send + Sync + 'static,
    {
        self.on_submit = Some(Arc::new(f));
        self
    }

    pub fn class_form(mut self, c: &str) -> Self {
        self.class_form = c.to_string();
        self
    }

    pub fn class_field(mut self, c: &str) -> Self {
        self.class_field = c.to_string();
        self
    }

    pub fn class_submit(mut self, c: &str) -> Self {
        self.class_submit = c.to_string();
        self
    }

    pub fn into_view(self) -> AnyView {
        let initial_t = match self.initial {
            Some(v) => v,
            None => T::default(),
        };
        let initial_json = match serde_json::to_value(&initial_t) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("FormBuilder: serialize initial: {}", e);
                crate::cata_log!(Error, msg.clone());
                return error_view(msg);
            }
        };
        let initial_obj = match initial_json {
            Value::Object(map) => map,
            other => {
                let msg = format!("FormBuilder<T>: T must serialize to a JSON object, got {:?}", other);
                crate::cata_log!(Error, msg.clone());
                return error_view(msg);
            }
        };

        let plan = build_plan(&initial_obj, &self.fields);

        let pending = RwSignal::new(false);

        let mut text_signals: BTreeMap<String, RwSignal<String>> = BTreeMap::new();
        let mut bool_signals: BTreeMap<String, RwSignal<bool>> = BTreeMap::new();
        for entry in plan.iter() {
            match &entry.kind {
                InputKind::Checkbox => {
                    let initial_bool = matches!(&entry.initial, Value::Bool(true));
                    bool_signals.insert(entry.name.clone(), RwSignal::new(initial_bool));
                }
                InputKind::Text
                | InputKind::TextArea
                | InputKind::Number
                | InputKind::Date
                | InputKind::DateTime
                | InputKind::Email
                | InputKind::Password
                | InputKind::Hidden
                | InputKind::Select(_) => {
                    text_signals.insert(entry.name.clone(), RwSignal::new(stringify_initial(&entry.initial)));
                }
            }
        }

        let plan_for_submit = plan.clone();
        let initial_obj_for_submit = initial_obj.clone();
        let text_signals_for_submit = text_signals.clone();
        let bool_signals_for_submit = bool_signals.clone();
        let on_submit_cb = self.on_submit.clone();

        let form_class = compose_class(style::form, &self.class_form);
        let field_class = compose_class(style::field, &self.class_field);
        let submit_class = compose_class(style::submit, &self.class_submit);

        let submit_label = self.submit_label.clone();

        let on_submit_handler = move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            if pending.get_untracked() {
                return;
            }
            pending.set(true);
            let mut values: Map<String, Value> = Map::new();
            for entry in plan_for_submit.iter() {
                let raw = read_signal_value(entry, &text_signals_for_submit, &bool_signals_for_submit);
                let coerced = coerce_to_value(&raw, &entry.kind, initial_obj_for_submit.get(&entry.name));
                values.insert(entry.name.clone(), coerced);
            }
            let parsed = fields_to_t::<T>(&values);
            pending.set(false);
            match parsed {
                Ok(typed) => match on_submit_cb.clone() {
                    Some(callback) => {
                        leptos::task::spawn_local(async move {
                            callback(typed);
                        });
                    }
                    None => {}
                },
                Err(err) => {
                    crate::cata_log!(Error, format!("FormBuilder: deserialize submitted form: {}", err));
                }
            }
        };

        let fields_view = render_fields(plan.clone(), text_signals.clone(), bool_signals.clone(), field_class.clone());

        view! {
            <form class=form_class on:submit=on_submit_handler>
                {fields_view}
                <button type="submit" class=submit_class disabled=move || pending.get()>
                    {submit_label}
                </button>
            </form>
        }
        .into_any()
    }
}

fn build_plan(initial: &Map<String, Value>, fields: &BTreeMap<String, FieldMeta>) -> Vec<FormPlanEntry> {
    let mut out: Vec<FormPlanEntry> = Vec::new();
    for (name, value) in initial.iter() {
        let meta = fields.get(name);
        if meta.is_some_and(|m| m.ignored) {
            continue;
        }
        let user_set_kind = meta.is_some_and(|m| m.kind.is_some());
        if is_security_default_ignore(name) && !user_set_kind {
            continue;
        }
        let label = match meta.and_then(|m| m.label.clone()) {
            Some(l) => l,
            None => prettify_label(name),
        };
        let placeholder = meta.and_then(|m| m.placeholder.clone());
        let override_kind = meta.and_then(|m| m.kind.clone());
        let kind = match override_kind {
            Some(k) => k,
            None => infer_kind(name, value),
        };
        out.push(FormPlanEntry {
            name: name.clone(),
            label,
            placeholder,
            kind,
            initial: value.clone(),
        });
    }
    out
}

fn is_security_default_ignore(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    for suffix in PASSWORD_HIDE_SUFFIXES {
        if lower == *suffix || lower.ends_with(suffix) {
            return true;
        }
    }
    false
}

fn infer_kind(name: &str, value: &Value) -> InputKind {
    let lower = name.to_ascii_lowercase();
    if lower == "id" {
        return InputKind::Hidden;
    }
    if lower == "email" || lower.ends_with("_email") {
        return InputKind::Email;
    }
    for suffix in DATETIME_SUFFIXES {
        if lower.ends_with(suffix) {
            return InputKind::DateTime;
        }
    }
    match value {
        Value::Bool(_) => InputKind::Checkbox,
        Value::Number(_) => InputKind::Number,
        Value::String(s) => {
            if looks_like_iso_datetime(s) {
                InputKind::DateTime
            } else {
                InputKind::Text
            }
        }
        Value::Null => InputKind::Text,
        Value::Array(_) => InputKind::TextArea,
        Value::Object(_) => InputKind::TextArea,
    }
}

fn looks_like_iso_datetime(s: &str) -> bool {
    if s.len() < 16 {
        return false;
    }
    let bytes = s.as_bytes();
    if bytes.len() < 11 {
        return false;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if bytes[10] != b'T' && bytes[10] != b' ' {
        return false;
    }
    true
}

fn prettify_label(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut at_word_start = true;
    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            out.push(' ');
            at_word_start = true;
            continue;
        }
        if at_word_start {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
            at_word_start = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn stringify_initial(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) => match serde_json::to_string(value) {
            Ok(s) => s,
            Err(e) => {
                crate::cata_log!(Error, format!("FormBuilder: stringify nested initial: {}", e));
                String::new()
            }
        },
        Value::Object(_) => match serde_json::to_string(value) {
            Ok(s) => s,
            Err(e) => {
                crate::cata_log!(Error, format!("FormBuilder: stringify nested initial: {}", e));
                String::new()
            }
        },
    }
}

fn read_signal_value(entry: &FormPlanEntry, text: &BTreeMap<String, RwSignal<String>>, bools: &BTreeMap<String, RwSignal<bool>>) -> String {
    match &entry.kind {
        InputKind::Checkbox => match bools.get(&entry.name) {
            Some(sig) => sig.get_untracked().to_string(),
            None => String::from("false"),
        },
        InputKind::Text
        | InputKind::TextArea
        | InputKind::Number
        | InputKind::Date
        | InputKind::DateTime
        | InputKind::Email
        | InputKind::Password
        | InputKind::Hidden
        | InputKind::Select(_) => match text.get(&entry.name) {
            Some(sig) => sig.get_untracked(),
            None => String::from(""),
        },
    }
}

pub(crate) fn coerce_to_value(raw: &str, kind: &InputKind, original: Option<&Value>) -> Value {
    match kind {
        InputKind::Checkbox => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "" => Value::Bool(false),
            other => Value::Bool(!other.is_empty() && other != "0"),
        },
        InputKind::Number => coerce_number(raw, original),
        InputKind::Hidden => coerce_hidden(raw, original),
        InputKind::Text
        | InputKind::TextArea
        | InputKind::Date
        | InputKind::DateTime
        | InputKind::Email
        | InputKind::Password
        | InputKind::Select(_) => coerce_string(raw, original),
    }
}

fn coerce_string(raw: &str, original: Option<&Value>) -> Value {
    if raw.is_empty() {
        match original {
            Some(Value::Null) => Value::Null,
            Some(Value::Bool(_)) | Some(Value::Number(_)) | Some(Value::String(_)) | Some(Value::Array(_)) | Some(Value::Object(_)) | None => Value::String(String::new()),
        }
    } else {
        Value::String(raw.to_string())
    }
}

fn coerce_hidden(raw: &str, original: Option<&Value>) -> Value {
    match original {
        Some(Value::Number(_)) => coerce_number(raw, original),
        Some(Value::Bool(_)) => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            other => Value::Bool(!other.is_empty() && other != "0"),
        },
        Some(Value::Null) => {
            if raw.is_empty() {
                Value::Null
            } else {
                Value::String(raw.to_string())
            }
        }
        None => {
            if raw.is_empty() {
                Value::Null
            } else {
                Value::String(raw.to_string())
            }
        }
        Some(Value::String(_)) | Some(Value::Array(_)) | Some(Value::Object(_)) => Value::String(raw.to_string()),
    }
}

fn coerce_number(raw: &str, original: Option<&Value>) -> Value {
    let was_float = original.is_some_and(|v| match v {
        Value::Number(n) => n.is_f64() && !n.is_i64() && !n.is_u64(),
        Value::Null | Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => false,
    });
    if was_float {
        match raw.parse::<f64>() {
            Ok(f) => match Number::from_f64(f) {
                Some(num) => Value::Number(num),
                None => Value::Null,
            },
            Err(e) => {
                crate::cata_log!(Debug, format!("FormBuilder: parse f64 failed: {}", e));
                Value::Null
            }
        }
    } else {
        match raw.parse::<i64>() {
            Ok(i) => Value::Number(Number::from(i)),
            Err(e_i) => {
                crate::cata_log!(Debug, format!("FormBuilder: parse i64 failed (will fall back to f64): {}", e_i));
                match raw.parse::<f64>() {
                    Ok(f) => match Number::from_f64(f) {
                        Some(num) => Value::Number(num),
                        None => Value::Null,
                    },
                    Err(e_f) => {
                        crate::cata_log!(Debug, format!("FormBuilder: parse f64 failed: {}", e_f));
                        Value::Null
                    }
                }
            }
        }
    }
}

pub(crate) fn fields_to_t<T: DeserializeOwned>(values: &Map<String, Value>) -> Result<T, serde_json::Error> {
    serde_json::from_value::<T>(Value::Object(values.clone()))
}

fn compose_class(base: &str, extra: &str) -> String {
    if extra.is_empty() {
        base.to_string()
    } else {
        format!("{} {}", base, extra)
    }
}

fn error_view(msg: String) -> AnyView {
    crate::cata_log!(Error, msg.clone());
    view! { <p>{msg}</p> }.into_any()
}

fn render_fields(plan: Vec<FormPlanEntry>, text: BTreeMap<String, RwSignal<String>>, bools: BTreeMap<String, RwSignal<bool>>, field_class: String) -> AnyView {
    let nodes: Vec<AnyView> = plan
        .into_iter()
        .map(|entry| render_field(entry, &text, &bools, field_class.clone()))
        .collect();
    view! { <>{nodes}</> }.into_any()
}

fn render_field(entry: FormPlanEntry, text: &BTreeMap<String, RwSignal<String>>, bools: &BTreeMap<String, RwSignal<bool>>, field_class: String) -> AnyView {
    match entry.kind.clone() {
        InputKind::Hidden => render_hidden(entry, text),
        InputKind::Checkbox => render_checkbox(entry, bools, field_class),
        InputKind::TextArea => render_textarea(entry, text, field_class),
        InputKind::Select(options) => render_select(entry, text, options, field_class),
        InputKind::Text => render_input(entry, text, field_class, "text"),
        InputKind::Number => render_input(entry, text, field_class, "number"),
        InputKind::Date => render_input(entry, text, field_class, "date"),
        InputKind::DateTime => render_input(entry, text, field_class, "datetime-local"),
        InputKind::Email => render_input(entry, text, field_class, "email"),
        InputKind::Password => render_input(entry, text, field_class, "password"),
    }
}

fn render_hidden(entry: FormPlanEntry, text: &BTreeMap<String, RwSignal<String>>) -> AnyView {
    let sig = match text.get(&entry.name) {
        Some(s) => *s,
        None => RwSignal::new(stringify_initial(&entry.initial)),
    };
    let name = entry.name.clone();
    view! {
        <input
            type="hidden"
            name=name
            prop:value=move || sig.get()
        />
    }
    .into_any()
}

fn render_checkbox(entry: FormPlanEntry, bools: &BTreeMap<String, RwSignal<bool>>, field_class: String) -> AnyView {
    let sig = match bools.get(&entry.name) {
        Some(s) => *s,
        None => RwSignal::new(false),
    };
    let name = entry.name.clone();
    let label_text = entry.label.clone();
    view! {
        <div class=field_class>
            <label>
                <input
                    type="checkbox"
                    name=name
                    prop:checked=move || sig.get()
                    on:change=move |ev| sig.set(event_target_checked(&ev))
                />
                " "
                {label_text}
            </label>
        </div>
    }
    .into_any()
}

fn render_textarea(entry: FormPlanEntry, text: &BTreeMap<String, RwSignal<String>>, field_class: String) -> AnyView {
    let sig = match text.get(&entry.name) {
        Some(s) => *s,
        None => RwSignal::new(String::new()),
    };
    let name = entry.name.clone();
    let label_text = entry.label.clone();
    let placeholder_attr = match entry.placeholder.clone() {
        Some(p) => p,
        None => String::from(""),
    };
    view! {
        <div class=field_class>
            <label>{label_text}</label>
            <textarea
                name=name
                placeholder=placeholder_attr
                prop:value=move || sig.get()
                on:input=move |ev| sig.set(event_target_value(&ev))
            >
                {move || sig.get()}
            </textarea>
        </div>
    }
    .into_any()
}

fn render_select(entry: FormPlanEntry, text: &BTreeMap<String, RwSignal<String>>, options: Vec<(String, String)>, field_class: String) -> AnyView {
    let sig = match text.get(&entry.name) {
        Some(s) => *s,
        None => RwSignal::new(String::new()),
    };
    let name = entry.name.clone();
    let label_text = entry.label.clone();
    let opt_views: Vec<AnyView> = options
        .into_iter()
        .map(|(value, label)| {
            let value_for_attr = value.clone();
            let value_for_cmp = value.clone();
            view! {
                <option value=value_for_attr selected=move || sig.get() == value_for_cmp>
                    {label}
                </option>
            }
            .into_any()
        })
        .collect();
    view! {
        <div class=field_class>
            <label>{label_text}</label>
            <select
                name=name
                on:change=move |ev| sig.set(event_target_value(&ev))
            >
                {opt_views}
            </select>
        </div>
    }
    .into_any()
}

fn render_input(entry: FormPlanEntry, text: &BTreeMap<String, RwSignal<String>>, field_class: String, input_type: &'static str) -> AnyView {
    let sig = match text.get(&entry.name) {
        Some(s) => *s,
        None => RwSignal::new(String::new()),
    };
    let name = entry.name.clone();
    let label_text = entry.label.clone();
    let placeholder_attr = match entry.placeholder.clone() {
        Some(p) => p,
        None => String::from(""),
    };
    view! {
        <div class=field_class>
            <label>{label_text}</label>
            <input
                type=input_type
                name=name
                placeholder=placeholder_attr
                prop:value=move || sig.get()
                on:input=move |ev| sig.set(event_target_value(&ev))
            />
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
    struct Profile {
        id: i64,
        name: String,
        age: i64,
        active: bool,
        password: String,
    }

    #[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
    struct Note {
        id: i64,
        title: String,
        body: String,
        created_at: String,
    }

    fn sample_profile() -> Profile {
        Profile {
            id: 7,
            name: String::from("Alice"),
            age: 30,
            active: true,
            password: String::from("hunter2"),
        }
    }

    fn json_obj(t: &Profile) -> Map<String, Value> {
        match serde_json::to_value(t) {
            Ok(Value::Object(m)) => m,
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn plan_emits_all_visible_fields() {
        let initial = sample_profile();
        let obj = json_obj(&initial);
        let plan = build_plan(&obj, &BTreeMap::new());
        let names: Vec<&str> = plan.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"id"));
        assert!(names.contains(&"name"));
        assert!(names.contains(&"age"));
        assert!(names.contains(&"active"));
        assert!(!names.contains(&"password"), "password must be hidden by security default");
    }

    #[test]
    fn plan_uses_initial_values() {
        let initial = sample_profile();
        let obj = json_obj(&initial);
        let plan = build_plan(&obj, &BTreeMap::new());
        let name_entry = plan.iter().find(|e| e.name == "name").expect("name field");
        assert_eq!(name_entry.initial, Value::String(String::from("Alice")));
        let active_entry = plan.iter().find(|e| e.name == "active").expect("active field");
        assert_eq!(active_entry.initial, Value::Bool(true));
    }

    #[test]
    fn ignore_removes_field() {
        let initial = sample_profile();
        let obj = json_obj(&initial);
        let mut fields: BTreeMap<String, FieldMeta> = BTreeMap::new();
        fields.entry(String::from("age")).or_default().ignored = true;
        let plan = build_plan(&obj, &fields);
        let names: Vec<&str> = plan.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"age"));
    }

    #[test]
    fn label_override_replaces_default() {
        let initial = sample_profile();
        let obj = json_obj(&initial);
        let mut fields: BTreeMap<String, FieldMeta> = BTreeMap::new();
        fields.entry(String::from("name")).or_default().label = Some(String::from("Display Name"));
        let plan = build_plan(&obj, &fields);
        let entry = plan.iter().find(|e| e.name == "name").expect("name field");
        assert_eq!(entry.label, "Display Name");
    }

    #[test]
    fn input_kind_override_changes_type() {
        let initial = sample_profile();
        let obj = json_obj(&initial);
        let mut fields: BTreeMap<String, FieldMeta> = BTreeMap::new();
        fields.entry(String::from("name")).or_default().kind = Some(InputKind::TextArea);
        let plan = build_plan(&obj, &fields);
        let entry = plan.iter().find(|e| e.name == "name").expect("name field");
        assert!(matches!(entry.kind, InputKind::TextArea));
    }

    #[test]
    fn id_field_defaults_hidden() {
        let initial = sample_profile();
        let obj = json_obj(&initial);
        let plan = build_plan(&obj, &BTreeMap::new());
        let entry = plan.iter().find(|e| e.name == "id").expect("id field");
        assert!(matches!(entry.kind, InputKind::Hidden));
    }

    #[test]
    fn datetime_suffix_promotes_kind() {
        let initial = Note {
            id: 1,
            title: String::from("t"),
            body: String::from("b"),
            created_at: String::from("2026-04-01T10:30:00"),
        };
        let raw = match serde_json::to_value(&initial) {
            Ok(Value::Object(m)) => m,
            _ => panic!("expected object"),
        };
        let plan = build_plan(&raw, &BTreeMap::new());
        let entry = plan.iter().find(|e| e.name == "created_at").expect("created_at field");
        assert!(matches!(entry.kind, InputKind::DateTime));
    }

    #[test]
    fn fields_to_t_round_trips_typed_values() {
        let initial = sample_profile();
        let mut values: Map<String, Value> = Map::new();
        values.insert(String::from("id"), Value::Number(Number::from(7i64)));
        values.insert(String::from("name"), Value::String(String::from("Alice")));
        values.insert(String::from("age"), Value::Number(Number::from(30i64)));
        values.insert(String::from("active"), Value::Bool(true));
        values.insert(String::from("password"), Value::String(String::from("hunter2")));
        let parsed: Profile = fields_to_t(&values).expect("parse profile");
        assert_eq!(parsed, initial);
    }

    #[test]
    fn coerce_number_handles_int_and_float() {
        let int_orig = Value::Number(Number::from(0i64));
        let v = coerce_to_value("42", &InputKind::Number, Some(&int_orig));
        assert_eq!(v, Value::Number(Number::from(42i64)));

        let float_orig = match Number::from_f64(0.5) {
            Some(n) => Value::Number(n),
            None => panic!("float make"),
        };
        let v = coerce_to_value("3.14", &InputKind::Number, Some(&float_orig));
        match v {
            Value::Number(n) => assert!((n.as_f64().expect("f64") - 3.14).abs() < 1e-9),
            other => panic!("expected number, got {:?}", other),
        }
    }

    #[test]
    fn coerce_checkbox_handles_strings() {
        assert_eq!(coerce_to_value("true", &InputKind::Checkbox, None), Value::Bool(true));
        assert_eq!(coerce_to_value("false", &InputKind::Checkbox, None), Value::Bool(false));
        assert_eq!(coerce_to_value("", &InputKind::Checkbox, None), Value::Bool(false));
        assert_eq!(coerce_to_value("on", &InputKind::Checkbox, None), Value::Bool(true));
    }

    #[test]
    fn submit_round_trip_via_helper() {
        let mut values: Map<String, Value> = Map::new();
        values.insert(String::from("id"), coerce_to_value("11", &InputKind::Hidden, Some(&Value::Number(Number::from(0i64)))));
        values.insert(String::from("name"), coerce_to_value("Bob", &InputKind::Text, Some(&Value::String(String::new()))));
        values.insert(String::from("age"), coerce_to_value("99", &InputKind::Number, Some(&Value::Number(Number::from(0i64)))));
        values.insert(String::from("active"), coerce_to_value("false", &InputKind::Checkbox, None));
        values.insert(String::from("password"), coerce_to_value("p", &InputKind::Password, None));
        let parsed: Profile = fields_to_t(&values).expect("parse profile");
        assert_eq!(parsed.id, 11);
        assert_eq!(parsed.name, "Bob");
        assert_eq!(parsed.age, 99);
        assert!(!parsed.active);
        assert_eq!(parsed.password, "p");
    }
}
