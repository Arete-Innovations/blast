pub mod leptos;

#[cfg(not(target_arch = "wasm32"))]
pub mod fuses;

#[cfg(not(target_arch = "wasm32"))]
pub mod http;

#[cfg(not(target_arch = "wasm32"))]
pub mod ws;
