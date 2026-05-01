use std::collections::HashMap;

use leptos::prelude::{Memo, RwSignal};

#[derive(Clone, Copy)]
pub struct QueryDialog {
    pub name: &'static str,
    pub visible: Memo<bool>,
    pub id: Memo<Option<i64>>,
}

#[derive(Clone, Copy)]
pub struct UrlListState {
    pub page: RwSignal<u64>,
    pub page_size: RwSignal<u64>,
    pub sort: RwSignal<Option<String>>,
    pub filter: RwSignal<HashMap<String, String>>,
}
