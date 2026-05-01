#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteName {
    Welcome,
    Login,
    Logout,
    Register,
    Dashboard,
    Profile,
}

impl RouteName {
    pub fn path(self) -> &'static str {
        match self {
            RouteName::Welcome => "/",
            RouteName::Login => "/login",
            RouteName::Logout => "/logout",
            RouteName::Register => "/register",
            RouteName::Dashboard => "/dashboard",
            RouteName::Profile => "/profile",
        }
    }
}
