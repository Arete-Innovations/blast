//! Catalyst session primitives.
//!
//! This module owns:
//! - [`SessionContext`]: the framework-agnostic session record stuffed into
//!   request extensions by the auth middleware. Holds primitive identity data
//!   (`session_id`, `user_id`, `role`, `token`) so catalyst never needs to
//!   know the user-app's concrete `User` row shape.
//! - [`SessionUser`] and [`SessionAdapter`] traits: the contract user-apps
//!   implement (or have generated) so catalyst can lookup, create, and revoke
//!   sessions against arbitrary user tables.

pub mod traits;

pub use traits::{SessionAdapter, SessionUser};

/// Request-scoped session record stored in axum extensions after auth.
///
/// Holds only the primitives catalyst needs to enforce auth/role policy
/// plus the raw bearer token (so logout flows can revoke it). User-app
/// code that needs the full `User` row should fetch it from its own
/// model layer keyed by `user_id`.
#[derive(Debug, Clone)]
pub struct SessionContext {
    pub session_id: i64,
    pub user_id: i64,
    pub role: String,
    pub token: String,
}

impl SessionContext {
    pub fn new(
        session_id: i64,
        user_id: i64,
        role: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            user_id,
            role: role.into(),
            token: token.into(),
        }
    }

    /// Build a `SessionContext` from a `SessionUser` impl plus the matching
    /// session row id and bearer token. Convenience for adapters/middleware
    /// that hold the concrete user before erasing it.
    pub fn from_user<U: SessionUser>(
        session_id: i64,
        user: &U,
        token: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            user_id: user.id(),
            role: user.role().to_string(),
            token: token.into(),
        }
    }

    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.role == role
    }
}
