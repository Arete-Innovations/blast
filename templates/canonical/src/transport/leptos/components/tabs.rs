use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use stylance::import_crate_style;

use crate::structs::leptos::TabItem;

import_crate_style!(style, "src/transport/leptos/components/tabs.module.scss");

fn first_tab_key(items: &[TabItem]) -> String {
    let mut out = String::new();
    for item in items.iter().take(1) {
        out.push_str(&item.key);
    }
    out
}

fn read_tab_or_default(query_tab: Option<String>, default_key: &str) -> String {
    let raw = match query_tab {
        Some(s) => s,
        None => return default_key.to_string(),
    };
    match raw.is_empty() {
        true => default_key.to_string(),
        false => raw,
    }
}

fn build_tab_url(pathname: &str, query: &str, hash: &str, key: &str) -> String {
    let pairs = parse_query(query);
    let mut filtered: Vec<(String, String)> = Vec::new();
    for (k, v) in pairs {
        match k == "tab" {
            true => continue,
            false => filtered.push((k, v)),
        }
    }
    filtered.push(("tab".to_string(), key.to_string()));
    let qs = render_qs(&filtered);
    format!("{}{}{}", pathname, qs, hash)
}

fn parse_query(qs: &str) -> Vec<(String, String)> {
    let trimmed = match qs.strip_prefix('?') {
        Some(rest) => rest,
        None => qs,
    };
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for pair in trimmed.split('&') {
        if pair.is_empty() {
            continue;
        }
        let entry = match pair.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (pair.to_string(), String::new()),
        };
        out.push(entry);
    }
    out
}

fn render_qs(pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return String::new();
    }
    let mut out = String::from("?");
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        out.push_str(k);
        out.push('=');
        out.push_str(v);
    }
    out
}

#[component]
pub fn Tabs(items: Vec<TabItem>) -> impl IntoView {
    let default_key = first_tab_key(&items);
    let default_for_active = default_key.clone();
    let query_map = use_query_map();
    let active = Memo::new(move |_| {
        let raw = query_map.with(|m| m.get_str("tab").map(|s| s.to_string()));
        read_tab_or_default(raw, &default_for_active)
    });

    let stored_items = StoredValue::new(items);

    view! {
        <div class=style::wrap>
            <div class=style::tablist role="tablist">
                {move || {
                    let active_key = active.get();
                    stored_items.with_value(|items| items.clone()).into_iter().map(|item| {
                        let item_key = item.key.clone();
                        let key_for_class = item.key.clone();
                        let key_for_aria = item.key.clone();
                        let active_for_class = active_key.clone();
                        let active_for_aria = active_key.clone();
                        let on_click = {
                            let target_key = item_key.clone();
                            move |_| navigate_to_tab(&target_key)
                        };
                        let class_fn = move || match key_for_class == active_for_class {
                            true => format!("{} {}", style::tab, style::tab_active),
                            false => style::tab.to_string(),
                        };
                        let aria_selected = move || match key_for_aria == active_for_aria {
                            true => "true",
                            false => "false",
                        };
                        view! {
                            <button
                                type="button"
                                role="tab"
                                class=class_fn
                                aria-selected=aria_selected
                                on:click=on_click
                            >
                                {item.label}
                            </button>
                        }
                    }).collect_view()
                }}
            </div>
        </div>
    }
}

fn navigate_to_tab(key: &str) {
    let location = use_location();
    let pathname = location.pathname.get_untracked();
    let hash = location.hash.get_untracked();
    let query = location.query.get_untracked().to_query_string();
    let new_url = build_tab_url(&pathname, &query, &hash, key);
    let navigate = use_navigate();
    let opts = NavigateOptions {
        replace: true,
        scroll: false,
        ..Default::default()
    };
    navigate(&new_url, opts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_tab_picks_first_key() {
        let items = vec![TabItem::new("a", "A"), TabItem::new("b", "B")];
        assert_eq!(first_tab_key(&items), "a");
    }

    #[test]
    fn first_tab_empty_when_no_items() {
        let items: Vec<TabItem> = Vec::new();
        assert_eq!(first_tab_key(&items), "");
    }

    #[test]
    fn read_tab_uses_default_when_missing() {
        assert_eq!(read_tab_or_default(None, "x"), "x");
    }

    #[test]
    fn read_tab_uses_default_when_empty() {
        assert_eq!(read_tab_or_default(Some(String::new()), "x"), "x");
    }

    #[test]
    fn read_tab_returns_value_when_present() {
        assert_eq!(read_tab_or_default(Some("y".to_string()), "x"), "y");
    }

    #[test]
    fn build_url_replaces_existing_tab() {
        let url = build_tab_url("/p", "?tab=a&n=1", "", "b");
        assert!(url.contains("tab=b"));
        assert!(url.contains("n=1"));
        assert!(!url.contains("tab=a"));
    }
}
