use crate::structs::auth::SessionContext;

pub struct AdminGuard(pub SessionContext);
pub struct UserGuard(pub SessionContext);
pub struct Referer(pub String);
