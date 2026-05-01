use wasm_bindgen::prelude::wasm_bindgen;

use crate::transport::leptos::app::App;

#[wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
