pub mod role;
pub mod session_context;

pub use role::Role;
pub use session_context::SessionContext;

#[cfg(not(target_arch = "wasm32"))]
pub mod login;
#[cfg(not(target_arch = "wasm32"))]
pub mod register;
#[cfg(not(target_arch = "wasm32"))]
pub mod sessions;
#[cfg(not(target_arch = "wasm32"))]
pub mod users;

#[cfg(not(target_arch = "wasm32"))]
pub use login::{AuthResponse, LoginBody, LoginInput, LoginOutput};
#[cfg(not(target_arch = "wasm32"))]
pub use register::{RegisterBody, RegisterInput, RegisterOutput};
#[cfg(not(target_arch = "wasm32"))]
pub use sessions::{NewSession, Session, SESSION_TTL_SECS};
#[cfg(not(target_arch = "wasm32"))]
pub use users::{NewUser, User, UserPublic};
