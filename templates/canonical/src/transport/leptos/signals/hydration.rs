use leptos::prelude::*;

pub fn provide_hydration_store() -> RwSignal<bool> {
    let signal: RwSignal<bool> = RwSignal::new(false);
    provide_context(signal);
    spawn_post_mount(signal);
    signal
}

pub fn use_hydration() -> RwSignal<bool> {
    match use_context::<RwSignal<bool>>() {
        Some(signal) => signal,
        None => {
            let signal: RwSignal<bool> = RwSignal::new(false);
            provide_context(signal);
            spawn_post_mount(signal);
            signal
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn_post_mount(signal: RwSignal<bool>) {
    Effect::new(move |_| {
        signal.set(true);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_post_mount(_signal: RwSignal<bool>) {}
