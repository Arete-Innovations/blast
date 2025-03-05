use crate::middleware::*;
use crate::routes::*;
use crate::services::*;
use crate::structs::*;
use rocket::http::CookieJar;
use rocket::request::FlashMessage;
use rocket::response::Flash;
use rocket::response::Redirect;
use rocket::uri;
use rocket::{get, routes, Route};
use rocket_dyn_templates::Template;

#[get("/")]
pub fn get_home(cookies: &CookieJar<'_>, flash: Option<FlashMessage<'_>>, jwt: Option<JWT>) -> Result<Template, Flash<Redirect>> {
    if let Some(jwt) = jwt {
        if let Ok(user) = jwt_to_user(&jwt.0.sub) {
            let redirect_uri = if Users::is_admin(user.id) {
                uri!(private::admin::get_dashboard)
            } else {
                uri!(private::user::get_dashboard)
            };
            return Err(Flash::success(Redirect::to(redirect_uri), "Already logged in."));
        }
    }
    let path = "index";
    Ok(Template::render(path, &BaseContext::build(path, cookies, flash)))
}

#[get("/oops")]
pub fn page_not_found(cookies: &CookieJar<'_>, flash: Option<FlashMessage<'_>>) -> Template {
    let path = "oops/index";
    Template::render(path, BaseContext::build(path, cookies, flash))
}

pub fn routes() -> Vec<Route> {
    routes![get_home, page_not_found]
}
