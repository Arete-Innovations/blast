use crate::middleware::*;
use crate::services::*;
use crate::structs::*;
use rocket::http::CookieJar;
use rocket::request::FlashMessage;
use rocket::{get, routes, Route};
use rocket_dyn_templates::Template;

#[get("/user")]
pub fn get_dashboard(cookies: &CookieJar<'_>, jwt: JWT, flash: Option<FlashMessage<'_>>) -> Template {
    let path = "user/index";
    let user = Users::get_user_by_id(jwt_to_id(&jwt).unwrap()).unwrap();
    let base_ctx = BaseContext::build(path, cookies, flash);
    let ctx = base_ctx.with_extra(user);
    Template::render(path, &ctx)
}

pub fn routes() -> Vec<Route> {
    routes![get_dashboard]
}
