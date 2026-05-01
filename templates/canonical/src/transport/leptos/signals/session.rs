use leptos::prelude::*;

use crate::structs::auth::SessionContext;
use crate::structs::leptos::SessionStore;

pub fn provide_session_store() -> SessionStore {
    let initial: Option<SessionContext> = boot_session();
    let store = SessionStore::with_initial(initial);
    provide_context(store);
    store
}

#[cfg(not(target_arch = "wasm32"))]
fn boot_session() -> Option<SessionContext> {
    use_context::<crate::Ctx>().and_then(|ctx| ctx.session().cloned())
}

#[cfg(target_arch = "wasm32")]
fn boot_session() -> Option<SessionContext> {
    use crate::meltdown::{MeltDown, MeltType};
    let window = match web_sys::window() {
        Some(w) => w,
        None => return None,
    };
    let val = match js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("__cata_session")) {
        Ok(v) => v,
        Err(err) => {
            MeltDown::new(MeltType::Unexpected("session_boot_reflect".to_string()), format!("reflect get failed: {:?}", err)).log();
            return None;
        }
    };
    if val.is_null() || val.is_undefined() {
        return None;
    }
    let json_jsval = match js_sys::JSON::stringify(&val) {
        Ok(j) => j,
        Err(err) => {
            MeltDown::new(MeltType::Unexpected("session_boot_stringify".to_string()), format!("JSON.stringify failed: {:?}", err)).log();
            return None;
        }
    };
    let json_str = match json_jsval.as_string() {
        Some(s) => s,
        None => return None,
    };
    match serde_json::from_str::<Option<SessionContext>>(&json_str) {
        Ok(opt) => opt,
        Err(err) => {
            MeltDown::new(MeltType::Unexpected("session_boot_parse".to_string()), format!("deserialize failed: {}", err)).log();
            None
        }
    }
}

pub fn use_session() -> SessionStore {
    match use_context::<SessionStore>() {
        Some(store) => store,
        None => {
            let store = SessionStore::new();
            provide_context(store);
            store
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn ssr_session_payload() -> String {
    use crate::cata_log;
    let session: Option<SessionContext> = use_context::<crate::Ctx>().and_then(|ctx| ctx.session().cloned());
    let json: String = match serde_json::to_string(&session) {
        Ok(s) => s,
        Err(err) => {
            cata_log!(Warning, format!("session payload serialize failed: {}", err));
            "null".to_string()
        }
    };
    format!("window.__cata_session = {};", json)
}

#[cfg(target_arch = "wasm32")]
pub fn ssr_session_payload() -> String {
    String::new()
}
