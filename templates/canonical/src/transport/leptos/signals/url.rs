use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::NavigateOptions;

use crate::cata_log;
use crate::structs::leptos::{QueryDialog, UrlListState};
use crate::structs::list_query::{ListQuery, Sort, SortDirection};

impl QueryDialog {
    pub fn open(&self, id: Option<i64>) {
        let name = self.name;
        replace_query(move |params| {
            params.upsert("dialog".to_string(), name.to_string());
            match id {
                Some(n) => {
                    params.upsert("dialog_id".to_string(), n.to_string());
                }
                None => {
                    params.drop_key("dialog_id");
                }
            }
        });
    }

    pub fn close(&self) {
        replace_query(|params| {
            params.drop_key("dialog");
            params.drop_key("dialog_id");
        });
    }
}

#[track_caller]
pub fn use_query_dialog(name: &'static str) -> QueryDialog {
    let query_map = use_query_map();
    let visible = Memo::new(move |_| query_map.with(|map| map.get_str("dialog").is_some_and(|v| v == name)));
    let id = Memo::new(move |_| query_map.with(|map| parse_dialog_id(map.get_str("dialog_id"))));
    QueryDialog { name, visible, id }
}

fn parse_dialog_id(raw: Option<&str>) -> Option<i64> {
    let s = raw?;
    match s.parse::<i64>() {
        Ok(n) => Some(n),
        Err(e) => {
            cata_log!(Debug, format!("dialog_id parse failed for '{}': {}", s, e));
            None
        }
    }
}

impl UrlListState {
    pub fn to_list_query(&self) -> ListQuery {
        let page = self.page.get();
        let page_size = self.page_size.get();
        let sort_str = self.sort.get();
        let filter_map = self.filter.get();

        let sort = parse_sort_string(sort_str.as_deref());

        let mut filter: Vec<(String, String)> = filter_map.into_iter().collect();
        filter.sort_by(|a, b| a.0.cmp(&b.0));

        ListQuery {
            page: clamp_page(page),
            page_size: clamp_page_size(page_size),
            sort,
            filter,
        }
    }
}

fn parse_sort_string(raw: Option<&str>) -> Vec<Sort> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    raw.split(',')
        .filter_map(|seg| {
            let trimmed = seg.trim();
            if trimmed.is_empty() {
                return None;
            }
            let Some(rest) = trimmed.strip_prefix('-') else {
                return Some(Sort {
                    column: trimmed.to_string(),
                    direction: SortDirection::Asc,
                });
            };
            if rest.is_empty() {
                return None;
            }
            Some(Sort {
                column: rest.to_string(),
                direction: SortDirection::Desc,
            })
        })
        .collect()
}

#[track_caller]
pub fn use_url_list_state() -> UrlListState {
    let query_map = use_query_map();

    let initial = query_map.get_untracked();
    let page = RwSignal::new(read_page(&initial));
    let page_size = RwSignal::new(read_page_size(&initial));
    let sort = RwSignal::new(read_sort(&initial));
    let filter = RwSignal::new(read_filter(&initial));

    Effect::new(move |_| {
        if IS_NAVIGATING.load(Ordering::Relaxed) {
            return;
        }
        let map = query_map.get();
        let next_page = read_page(&map);
        if page.get_untracked() != next_page {
            page.set(next_page);
        }
        let next_page_size = read_page_size(&map);
        if page_size.get_untracked() != next_page_size {
            page_size.set(next_page_size);
        }
        let next_sort = read_sort(&map);
        if sort.get_untracked() != next_sort {
            sort.set(next_sort);
        }
        let next_filter = read_filter(&map);
        if filter.get_untracked() != next_filter {
            filter.set(next_filter);
        }
    });

    Effect::new(move |_| {
        let p = page.get();
        let ps = page_size.get();
        let s = sort.get();
        let f = filter.get();
        if IS_NAVIGATING.load(Ordering::Relaxed) {
            return;
        }
        push_list_to_url(p, ps, s.as_deref(), &f);
    });

    UrlListState {
        page,
        page_size,
        sort,
        filter,
    }
}

const DEFAULT_PAGE: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 25;
const MAX_PAGE_SIZE: u64 = 200;

fn clamp_page(p: u64) -> u32 {
    let n = if p == 0 { DEFAULT_PAGE } else { p };
    n.min(u32::MAX as u64) as u32
}

fn clamp_page_size(ps: u64) -> u32 {
    let n = match ps {
        0 => DEFAULT_PAGE_SIZE,
        n if n > MAX_PAGE_SIZE => MAX_PAGE_SIZE,
        n => n,
    };
    n as u32
}

fn read_page(map: &leptos_router::params::ParamsMap) -> u64 {
    let Some(s) = map.get_str("page") else {
        return DEFAULT_PAGE;
    };
    match s.parse::<u64>() {
        Ok(n) if n > 0 => n,
        Ok(n) => {
            cata_log!(Debug, format!("read_page: zero/invalid page value parsed as '{}', falling back", n));
            DEFAULT_PAGE
        }
        Err(e) => {
            cata_log!(Debug, format!("read_page: parse failed for '{}': {}", s, e));
            DEFAULT_PAGE
        }
    }
}

fn read_page_size(map: &leptos_router::params::ParamsMap) -> u64 {
    let Some(s) = map.get_str("page_size") else {
        return DEFAULT_PAGE_SIZE;
    };
    match s.parse::<u64>() {
        Ok(n) if n > 0 => n.min(MAX_PAGE_SIZE),
        Ok(n) => {
            cata_log!(Debug, format!("read_page_size: zero/invalid value parsed as '{}', falling back", n));
            DEFAULT_PAGE_SIZE
        }
        Err(e) => {
            cata_log!(Debug, format!("read_page_size: parse failed for '{}': {}", s, e));
            DEFAULT_PAGE_SIZE
        }
    }
}

fn read_sort(map: &leptos_router::params::ParamsMap) -> Option<String> {
    let s = map.get_str("sort")?;
    if s.is_empty() {
        return None;
    }
    Some(s.to_string())
}

fn read_filter(map: &leptos_router::params::ParamsMap) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let pairs = map_pairs(map);
    for (k, v) in pairs {
        let Some(rest) = k.strip_prefix("filter[") else {
            continue;
        };
        let Some(col) = rest.strip_suffix(']') else {
            continue;
        };
        if col.is_empty() {
            continue;
        }
        out.insert(col.to_string(), v);
    }
    out
}

fn map_pairs(map: &leptos_router::params::ParamsMap) -> Vec<(String, String)> {
    let qs = map.to_query_string();
    if qs.is_empty() {
        return Vec::new();
    }
    let trimmed = strip_leading_question(&qs);
    trimmed.split('&').filter(|s| !s.is_empty()).map(decode_pair).collect()
}

fn strip_leading_question(qs: &str) -> &str {
    match qs.strip_prefix('?') {
        Some(rest) => rest,
        None => qs,
    }
}

fn decode_pair(pair: &str) -> (String, String) {
    match pair.split_once('=') {
        Some((k, v)) => (decode(k), decode(v)),
        None => {
            let only = decode(pair);
            (only, String::new())
        }
    }
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b as char);
                        i += 3;
                    }
                    Err(e) => {
                        cata_log!(Debug, format!("decode: malformed %-escape '{}': {}", hex, e));
                        out.push('%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

fn push_list_to_url(page: u64, page_size: u64, sort: Option<&str>, filter: &HashMap<String, String>) {
    let location = use_location();
    let pathname = location.pathname.get_untracked();
    let hash = location.hash.get_untracked();
    let mut preserved: Vec<(String, String)> = Vec::new();
    let map = location.query.get_untracked();
    for (k, v) in map_pairs(&map) {
        if k == "page" || k == "page_size" || k == "sort" {
            continue;
        }
        if k.starts_with("filter[") && k.ends_with(']') {
            continue;
        }
        preserved.push((k, v));
    }

    let mut emitted: Vec<(String, String)> = preserved;
    if page != DEFAULT_PAGE {
        emitted.push(("page".to_string(), page.to_string()));
    }
    if page_size != DEFAULT_PAGE_SIZE {
        emitted.push(("page_size".to_string(), page_size.to_string()));
    }
    match sort {
        Some(s) => {
            if !s.is_empty() {
                emitted.push(("sort".to_string(), s.to_string()));
            }
        }
        None => {}
    }
    let mut filter_keys: Vec<&String> = filter.keys().collect();
    filter_keys.sort();
    for k in filter_keys {
        let Some(v) = filter.get(k) else {
            continue;
        };
        emitted.push((format!("filter[{}]", k), v.clone()));
    }

    let qs = render_qs(&emitted);
    let new_url = format!("{}{}{}", pathname, qs, hash);

    let current = location.query.get_untracked().to_query_string();
    let current_full = format!("{}{}{}", pathname, current, hash);
    if current_full == new_url {
        return;
    }

    do_navigate(&new_url);
}

fn replace_query(mutate: impl FnOnce(&mut Vec<(String, String)>)) {
    let location = use_location();
    let pathname = location.pathname.get_untracked();
    let hash = location.hash.get_untracked();
    let mut pairs = map_pairs(&location.query.get_untracked());
    mutate(&mut pairs);

    let qs = render_qs(&pairs);
    let new_url = format!("{}{}{}", pathname, qs, hash);
    do_navigate(&new_url);
}

fn render_qs(pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return String::new();
    }
    let mut qs = String::from("?");
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            qs.push('&');
        }
        qs.push_str(&encode(k));
        qs.push('=');
        qs.push_str(&encode(v));
    }
    qs
}

fn do_navigate(new_url: &str) {
    IS_NAVIGATING.store(true, Ordering::Relaxed);
    let navigate = use_navigate();
    let opts = NavigateOptions {
        replace: true,
        scroll: false,
        ..Default::default()
    };
    navigate(new_url, opts);
    IS_NAVIGATING.store(false, Ordering::Relaxed);
}

trait MutVecExt {
    fn upsert(&mut self, key: String, value: String);
    fn drop_key(&mut self, key: &str);
}

impl MutVecExt for Vec<(String, String)> {
    fn upsert(&mut self, key: String, value: String) {
        let mut found = false;
        for slot in self.iter_mut() {
            if slot.0 == key {
                slot.1 = value.clone();
                found = true;
                break;
            }
        }
        if !found {
            self.push((key, value));
        }
    }

    fn drop_key(&mut self, key: &str) {
        self.retain(|pair| pair.0 != key);
    }
}

static IS_NAVIGATING: AtomicBool = AtomicBool::new(false);
