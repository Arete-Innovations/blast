#[cfg(target_arch = "wasm32")]
use gloo_timers::callback::Interval;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct PolledState {
    pub interval: Option<Interval>,
    pub visibility_listener: Option<Closure<dyn FnMut()>>,
}
