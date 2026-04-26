
pub mod handlers;
pub mod router;
pub mod schema_view;
pub mod templates;

pub use router::{admin_router, admin_router_with};
pub use schema_view::{AdminColumn, AdminConfig, AdminTable};
