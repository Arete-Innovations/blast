use std::collections::HashMap;

use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::{FilterDef, FilterKind};
use crate::transport::leptos::signals::use_url_list_state;

import_crate_style!(style, "src/transport/leptos/components/filter_bar.module.scss");

#[cfg(target_arch = "wasm32")]
const DEBOUNCE_MS: u32 = 250;

fn placeholder_or_empty(input: Option<&String>) -> String {
    let mut out = String::new();
    for s in input.iter() {
        out.push_str(s);
    }
    out
}

fn filter_value_or_empty(map: &HashMap<String, String>, column: &str) -> String {
    let mut out = String::new();
    for v in map.get(column).iter() {
        out.push_str(v);
    }
    out
}

fn apply_filter_value(state_filter: RwSignal<HashMap<String, String>>, page: RwSignal<u64>, column: String, value: String) {
    state_filter.update(|map| {
        match value.is_empty() {
            true => {
                map.remove(&column);
            }
            false => {
                map.insert(column, value);
            }
        }
    });
    page.set(1);
}

#[component]
pub fn FilterBar(filters: Vec<FilterDef>) -> impl IntoView {
    let url_state = use_url_list_state();
    let filter_signal = url_state.filter;
    let page_signal = url_state.page;

    view! {
        <div class=style::wrap>
            {filters.into_iter().map(|f| {
                let column = f.column.clone();
                let label = f.label.clone();
                let placeholder = placeholder_or_empty(f.placeholder.as_ref());
                let initial = filter_signal.with_untracked(|m| filter_value_or_empty(m, &column));
                let render_kind = f.kind.clone();
                let column_for_input = column.clone();
                view! {
                    <label class=style::field>
                        <span class=style::label>{label}</span>
                        {render_filter_input(render_kind, column_for_input, initial, placeholder, filter_signal, page_signal)}
                    </label>
                }
            }).collect_view()}
        </div>
    }
}

fn render_filter_input(
    kind: FilterKind,
    column: String,
    initial: String,
    placeholder: String,
    filter_signal: RwSignal<std::collections::HashMap<String, String>>,
    page_signal: RwSignal<u64>,
) -> AnyView {
    match kind {
        FilterKind::Text => render_text_input(column, initial, placeholder, filter_signal, page_signal),
        FilterKind::Select(options) => render_select_input(column, initial, options, filter_signal, page_signal),
        FilterKind::Bool => render_bool_input(column, initial, filter_signal, page_signal),
    }
}

fn render_text_input(column: String, initial: String, placeholder: String, filter_signal: RwSignal<HashMap<String, String>>, page_signal: RwSignal<u64>) -> AnyView {
    let local = RwSignal::new(initial);

    #[cfg(target_arch = "wasm32")]
    setup_text_debounce(local, column, filter_signal, page_signal);

    #[cfg(not(target_arch = "wasm32"))]
    drop((column, filter_signal, page_signal));

    let on_input = move |ev: leptos::ev::Event| {
        let value = leptos::prelude::event_target_value(&ev);
        local.set(value);
    };
    let display = move || local.get();
    view! {
        <input
            class=style::input
            type="text"
            placeholder=placeholder
            prop:value=display
            on:input=on_input
        />
    }
    .into_any()
}

#[cfg(target_arch = "wasm32")]
fn setup_text_debounce(local: RwSignal<String>, column: String, filter_signal: RwSignal<HashMap<String, String>>, page_signal: RwSignal<u64>) {
    use std::cell::RefCell;
    use std::rc::Rc;

    let timer_slot: Rc<RefCell<Option<gloo_timers::callback::Timeout>>> = Rc::new(RefCell::new(Option::<gloo_timers::callback::Timeout>::None));
    let initial_run = StoredValue::new_local(true);
    Effect::new(move |_| {
        let value = local.get();
        if initial_run.get_value() {
            initial_run.set_value(false);
            return;
        }
        let column_inner = column.clone();
        let timer_for_callback = timer_slot.clone();
        let new_timeout = gloo_timers::callback::Timeout::new(DEBOUNCE_MS, move || {
            apply_filter_value(filter_signal, page_signal, column_inner.clone(), value.clone());
            timer_for_callback.borrow_mut().take();
        });
        let prev_timeout = timer_slot.borrow_mut().replace(new_timeout);
        match prev_timeout {
            Some(t) => t.cancel(),
            None => return,
        };
    });
}

fn render_select_input(
    column: String,
    initial: String,
    options: Vec<(String, String)>,
    filter_signal: RwSignal<std::collections::HashMap<String, String>>,
    page_signal: RwSignal<u64>,
) -> AnyView {
    let column_for_change = column.clone();
    let initial_value = initial.clone();
    let on_change = move |ev: leptos::ev::Event| {
        let value = leptos::prelude::event_target_value(&ev);
        apply_filter_value(filter_signal, page_signal, column_for_change.clone(), value);
    };
    view! {
        <select class=style::select on:change=on_change prop:value=initial_value.clone()>
            <option value="">"All"</option>
            {options.into_iter().map(|(value, label)| {
                view! {
                    <option value=value>{label}</option>
                }
            }).collect_view()}
        </select>
    }
    .into_any()
}

fn render_bool_input(
    column: String,
    initial: String,
    filter_signal: RwSignal<std::collections::HashMap<String, String>>,
    page_signal: RwSignal<u64>,
) -> AnyView {
    let initial_clone = initial.clone();
    let on_change = move |ev: leptos::ev::Event| {
        let value = leptos::prelude::event_target_value(&ev);
        apply_filter_value(filter_signal, page_signal, column.clone(), value);
    };
    view! {
        <select class=style::select on:change=on_change prop:value=initial_clone>
            <option value="">"Any"</option>
            <option value="true">"True"</option>
            <option value="false">"False"</option>
        </select>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_bar_text_kind() {
        let f = FilterDef::text("name", "Name");
        assert_eq!(f.column, "name");
        assert_eq!(f.label, "Name");
    }

    #[test]
    fn filter_bar_select_kind() {
        let f = FilterDef::select("status", "Status", vec![("a".into(), "Alpha".into())]);
        match f.kind {
            FilterKind::Select(options) => assert_eq!(options.len(), 1),
            FilterKind::Text => panic!("expected select"),
            FilterKind::Bool => panic!("expected select"),
        }
    }
}
