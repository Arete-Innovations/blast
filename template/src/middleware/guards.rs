use crate::structs::*;
use rocket::async_trait;
use rocket::http::Status;
use rocket::outcome::Outcome::{Error, Success};
use rocket::request::{FromRequest, Outcome, Request};

pub struct AdminGuard;

#[async_trait]
impl<'r> FromRequest<'r> for AdminGuard {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        if let Some(cookie) = req.cookies().get("user_id") {
            if let Ok(user_id) = cookie.value().parse::<i32>() {
                if Users::is_admin(user_id) {
                    return Success(AdminGuard);
                }
            }
        }
        Error((Status::Forbidden, ()))
    }
}

pub struct Referer(pub String);
#[async_trait]
impl<'r> FromRequest<'r> for Referer {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one("Referer") {
            Some(referer) => Outcome::Success(Referer(referer.to_string())),
            None => Outcome::Forward(Status::NotFound),
        }
    }
}
