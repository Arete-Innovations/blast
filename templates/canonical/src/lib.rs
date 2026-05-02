#![recursion_limit = "2048"]

pub mod meltdown;
pub mod structs;
pub mod transport;

pub mod cata_log;

#[cfg(not(target_arch = "wasm32"))]
pub mod bootstrap;

#[cfg(not(target_arch = "wasm32"))]
pub mod crank;

#[cfg(not(target_arch = "wasm32"))]
pub mod ctx;

#[cfg(not(target_arch = "wasm32"))]
pub mod database;

#[cfg(not(target_arch = "wasm32"))]
pub mod flows;

#[cfg(not(target_arch = "wasm32"))]
pub mod models;

#[cfg(not(target_arch = "wasm32"))]
pub mod routines;

pub mod services;

#[cfg(not(target_arch = "wasm32"))]
pub use bootstrap::bootstrap;

#[cfg(not(target_arch = "wasm32"))]
pub use ctx::Ctx;
