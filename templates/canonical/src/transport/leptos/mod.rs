pub mod app;
pub mod components;
pub mod pages;

#[cfg(target_arch = "wasm32")]
pub mod client;

pub use app::App;
