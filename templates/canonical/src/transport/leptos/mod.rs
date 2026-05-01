pub mod app;
pub mod components;
pub mod data;
pub mod pages;
pub mod routes;
pub mod signals;

#[cfg(target_arch = "wasm32")]
pub mod api_client;

#[cfg(target_arch = "wasm32")]
pub mod client;

pub use app::App;
