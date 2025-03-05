use crate::cata_log;
use crate::middleware::*;
use crate::services::*;
use crate::structs::*;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header as JWTHeader};
use rocket::form::Form;
use rocket::http::{Cookie, CookieJar};
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::uri;
use rocket::{get, post, routes, Route};
use rocket_dyn_templates::Template;
use std::env;

#[post("/login", data = "<login_form>")]
fn post_login(login_form: Form<LoginForm>, cookies: &CookieJar<'_>) -> Result<Flash<Redirect>, Flash<Redirect>> {
    let login = login_form.into_inner();
    if let Ok(user) = Users::get_user_by_username(&login.username) {
        if user.verify_password(&login.password) {
            let expiration = Utc::now().checked_add_signed(Duration::seconds(86400)).unwrap().timestamp();
            let claims = Claims {
                sub: user.id.to_string(),
                exp: expiration as usize,
            };
            let secret = env::var("JWT_SECRET").unwrap();
            let token = encode(&JWTHeader::default(), &claims, &EncodingKey::from_secret(secret.as_ref())).unwrap();

            cookies.add(Cookie::new("token", token));
            cookies.add(Cookie::new("user_id", user.id.to_string()));
            cata_log!(Info, format!("User {} logged in successfully", user.username));

            let redirect_uri = if Users::is_admin(user.id) {
                uri!(crate::routes::admin::get_dashboard)
            } else {
                uri!(crate::routes::user::get_dashboard)
            };
            return Ok(Flash::success(Redirect::to(redirect_uri), "Successfully logged in."));
        }
    }
    cata_log!(Warning, "Invalid login attempt");
    Err(Flash::error(Redirect::to(uri!(get_login)), "Invalid username or password."))
}

#[get("/logout")]
fn get_logout(cookies: &CookieJar<'_>) -> Flash<Redirect> {
    cookies.remove(Cookie::build("token"));
    cata_log!(Info, "User logged out");
    Flash::success(Redirect::to(uri!(get_login)), "Successfully logged out.")
}

#[get("/register")]
fn get_register(cookies: &CookieJar<'_>, flash: Option<FlashMessage<'_>>) -> Template {
    let path = "auth/register";
    cata_log!(Info, "Rendering registration page");
    let context = BaseContext::build(path, cookies, flash);
    Template::render(path, &context)
}

#[post("/register", data = "<register_form>")]
fn post_register(register_form: Form<RegisterForm>) -> Flash<Redirect> {
    let register = register_form.into_inner();

    match Users::register_user(register) {
        Ok(()) => {
            cata_log!(Info, "User registered successfully");
            Flash::success(Redirect::to(uri!(get_login)), "Successfully registered.")
        }
        Err(err_msg) => {
            cata_log!(Error, format!("Registration error: {}", err_msg));
            Flash::error(Redirect::to(uri!(get_register)), err_msg)
        }
    }
}

#[get("/login")]
fn get_login(cookies: &CookieJar<'_>, flash: Option<FlashMessage<'_>>, jwt: Option<JWT>) -> Result<Template, Flash<Redirect>> {
    if let Some(jwt) = jwt {
        match jwt_to_user(&jwt.0.sub) {
            Ok(user) => {
                let redirect_uri = if Users::is_admin(user.id) {
                    uri!(crate::routes::admin::get_dashboard)
                } else {
                    uri!(crate::routes::user::get_dashboard)
                };
                cata_log!(Info, format!("User {} is already logged in", user.username));
                return Err(Flash::success(Redirect::to(redirect_uri), "Already logged in."));
            }
            Err(status) => {
                cata_log!(Warning, format!("JWT invalid: {:?}", status));
            }
        }
    }
    let path = "auth/login";
    cata_log!(Info, "Rendering login page");
    Ok(Template::render(path, &BaseContext::build(path, cookies, flash)))
}

pub fn routes() -> Vec<Route> {
    routes![get_login, get_logout, get_register, post_login, post_register,]
}
