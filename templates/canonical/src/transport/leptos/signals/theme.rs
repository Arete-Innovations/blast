use leptos::prelude::*;

use crate::structs::leptos::Theme;

const THEME_COOKIE: &str = "theme";

pub fn provide_theme_store() -> RwSignal<Theme> {
    let initial = boot_theme();
    let signal = RwSignal::new(initial);
    provide_context(signal);

    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        let value = signal.get();
        apply_theme_to_dom(value);
        write_theme_cookie(value);
    });

    signal
}

pub fn use_theme() -> RwSignal<Theme> {
    match use_context::<RwSignal<Theme>>() {
        Some(signal) => signal,
        None => {
            let signal = RwSignal::new(Theme::System);
            provide_context(signal);
            signal
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn boot_theme() -> Theme {
    ssr_resolve_theme()
}

#[cfg(target_arch = "wasm32")]
fn boot_theme() -> Theme {
    match read_theme_from_document_cookie() {
        Some(theme) => theme,
        None => Theme::System,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn ssr_resolve_theme() -> Theme {
    use crate::cata_log;
    let parts = match use_context::<axum::http::request::Parts>() {
        Some(p) => p,
        None => return Theme::System,
    };
    let raw_value = match parts.headers.get(axum::http::header::COOKIE) {
        Some(v) => v,
        None => return Theme::System,
    };
    let raw = match raw_value.to_str() {
        Ok(s) => s,
        Err(err) => {
            cata_log!(Debug, format!("ssr non-utf8 cookie header: {}", err));
            return Theme::System;
        }
    };
    match cookie_value_from_header(raw, THEME_COOKIE) {
        Some(value) => Theme::from_cookie(&value),
        None => Theme::System,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn ssr_theme_str() -> &'static str {
    ssr_resolve_theme().as_str()
}

#[cfg(target_arch = "wasm32")]
pub fn ssr_theme_str() -> &'static str {
    "system"
}

#[cfg(not(target_arch = "wasm32"))]
fn cookie_value_from_header(header: &str, name: &str) -> Option<String> {
    for entry in header.split(';') {
        let trimmed = entry.trim();
        let (cookie_name, cookie_value) = match trimmed.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if cookie_name == name {
            let v = cookie_value.trim();
            if v.is_empty() {
                return None;
            }
            return Some(v.to_string());
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn read_theme_from_document_cookie() -> Option<Theme> {
    use crate::meltdown::{MeltDown, MeltType};
    use wasm_bindgen::JsCast;
    let window = web_sys::window()?;
    let document = window.document()?;
    let html_doc = match document.dyn_into::<web_sys::HtmlDocument>() {
        Ok(d) => d,
        Err(err) => {
            MeltDown::new(MeltType::Unexpected("theme_doc_dyn_into".to_string()), format!("HtmlDocument cast failed: {:?}", err)).log();
            return None;
        }
    };
    let raw = match html_doc.cookie() {
        Ok(c) => c,
        Err(err) => {
            MeltDown::new(MeltType::Unexpected("theme_cookie_read".to_string()), format!("document.cookie read failed: {:?}", err)).log();
            return None;
        }
    };
    for entry in raw.split(';') {
        let trimmed = entry.trim();
        let (name, value) = match trimmed.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if name == THEME_COOKIE {
            let v = value.trim();
            if v.is_empty() {
                return None;
            }
            return Some(Theme::from_cookie(v));
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn apply_theme_to_dom(theme: Theme) {
    use crate::meltdown::{MeltDown, MeltType};
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    let element = match document.document_element() {
        Some(e) => e,
        None => return,
    };
    if let Err(err) = element.set_attribute("data-theme", theme.as_str()) {
        MeltDown::new(MeltType::Unexpected("theme_set_attribute".to_string()), format!("setAttribute failed: {:?}", err)).log();
    }
}

#[cfg(target_arch = "wasm32")]
fn write_theme_cookie(theme: Theme) {
    use crate::meltdown::{MeltDown, MeltType};
    use wasm_bindgen::JsCast;
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    let html_doc = match document.dyn_into::<web_sys::HtmlDocument>() {
        Ok(d) => d,
        Err(err) => {
            MeltDown::new(MeltType::Unexpected("theme_doc_dyn_into_write".to_string()), format!("HtmlDocument cast failed: {:?}", err)).log();
            return;
        }
    };
    let value = format!("{}={}; SameSite=Lax; Path=/; Max-Age=31536000", THEME_COOKIE, theme.as_str());
    if let Err(err) = html_doc.set_cookie(&value) {
        MeltDown::new(MeltType::Unexpected("theme_cookie_write".to_string()), format!("document.cookie write failed: {:?}", err)).log();
    }
}
