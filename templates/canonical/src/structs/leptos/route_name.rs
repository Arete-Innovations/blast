use std::borrow::Cow;

#[derive(Debug, Clone)]
pub enum RouteName {
    Welcome,
    Login,
    Logout,
    Register,
    Dashboard,
    Profile,
    NotFound,
    ResourceList(&'static str),
    ResourceDetail(&'static str, i64),
    ResourceCreate(&'static str),
    ResourceEdit(&'static str, i64),
}

impl RouteName {
    pub fn path(&self) -> Cow<'static, str> {
        match self {
            RouteName::Welcome => Cow::Borrowed("/"),
            RouteName::Login => Cow::Borrowed("/login"),
            RouteName::Logout => Cow::Borrowed("/logout"),
            RouteName::Register => Cow::Borrowed("/register"),
            RouteName::Dashboard => Cow::Borrowed("/dashboard"),
            RouteName::Profile => Cow::Borrowed("/profile"),
            RouteName::NotFound => Cow::Borrowed("/404"),
            RouteName::ResourceList(r) => Cow::Owned(format!("/{r}")),
            RouteName::ResourceDetail(r, id) => Cow::Owned(format!("/{r}/{id}")),
            RouteName::ResourceCreate(r) => Cow::Owned(format!("/{r}/new")),
            RouteName::ResourceEdit(r, id) => Cow::Owned(format!("/{r}/{id}/edit")),
        }
    }
}
