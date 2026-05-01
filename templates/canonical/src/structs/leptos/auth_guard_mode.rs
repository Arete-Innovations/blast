#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthGuardMode {
    Public,
    AnonOnly,
    Required,
    AdminOnly,
}
