#[cfg(not(target_arch = "wasm32"))]
pub mod email;
#[cfg(not(target_arch = "wasm32"))]
pub mod rate_limit;
pub mod render;
#[cfg(not(target_arch = "wasm32"))]
pub mod storage;

#[cfg(not(target_arch = "wasm32"))]
pub use email::*;
#[cfg(not(target_arch = "wasm32"))]
pub use rate_limit::*;
pub use render::*;
#[cfg(not(target_arch = "wasm32"))]
pub use storage::*;
