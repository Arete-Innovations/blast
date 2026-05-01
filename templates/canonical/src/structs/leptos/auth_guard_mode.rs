#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthGuardMode {
    Public,
    Required,
    AdminOnly,
}
