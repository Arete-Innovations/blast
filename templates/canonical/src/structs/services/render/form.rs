use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;

use serde_json::Value;

pub type FormSubmitFn<T> = Arc<dyn Fn(T) + Send + Sync + 'static>;

#[derive(Clone)]
pub enum InputKind {
    Text,
    TextArea,
    Number,
    Checkbox,
    Date,
    DateTime,
    Email,
    Password,
    Hidden,
    Select(Vec<(String, String)>),
}

#[derive(Default, Clone)]
pub struct FieldMeta {
    pub label: Option<String>,
    pub placeholder: Option<String>,
    pub kind: Option<InputKind>,
    pub ignored: bool,
}

pub struct FormBuilder<T> {
    pub initial: Option<T>,
    pub fields: BTreeMap<String, FieldMeta>,
    pub submit_label: String,
    pub on_submit: Option<FormSubmitFn<T>>,
    pub class_form: String,
    pub class_field: String,
    pub class_submit: String,
    pub _phantom: PhantomData<T>,
}

impl<T> Default for FormBuilder<T> {
    fn default() -> Self {
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
}

#[derive(Clone)]
pub struct FormPlanEntry {
    pub name: String,
    pub label: String,
    pub placeholder: Option<String>,
    pub kind: InputKind,
    pub initial: Value,
}
