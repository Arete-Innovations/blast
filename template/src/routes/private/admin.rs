use crate::middleware::*;
use crate::services::*;
use rocket::http::CookieJar;
use rocket::request::FlashMessage;
use rocket::{get, routes, Route};
use rocket_dyn_templates::Template;

#[get("/admin")]
pub fn get_dashboard(_admin: AdminGuard, cookies: &CookieJar<'_>, _jwt: JWT, flash: Option<FlashMessage<'_>>) -> Template {
    let path = "admin/index";
    Template::render(path, BaseContext::build(path, cookies, flash))
}

pub fn routes() -> Vec<Route> {
    routes![get_dashboard]
}
