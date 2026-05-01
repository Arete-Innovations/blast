pub mod auth;
pub mod generated;
pub mod leptos;
pub mod list_query;
pub mod services;

#[cfg(not(target_arch = "wasm32"))]
pub mod fuses;
#[cfg(not(target_arch = "wasm32"))]
pub mod middleware;
#[cfg(not(target_arch = "wasm32"))]
pub mod ws;

#[cfg(not(target_arch = "wasm32"))]
pub use auth::{AuthResponse, LoginBody, LoginInput, LoginOutput, NewSession, NewUser, RegisterBody, RegisterInput, RegisterOutput, Session, User, UserPublic, SESSION_TTL_SECS};

pub use auth::{Role, SessionContext};

pub use list_query::*;
#[cfg(not(target_arch = "wasm32"))]
pub use services::*;
