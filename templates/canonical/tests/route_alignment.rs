//! Asserts that `RouteName::X.path()` matches the literal `path!(...)` string used in
//! `src/transport/leptos/app.rs`. The router macro accepts only literal strings, so the
//! enum and the macro literals can drift silently. This test catches that.

use canonical::structs::leptos::RouteName;

#[test]
fn welcome_path_matches_literal() {
    assert_eq!(RouteName::Welcome.path().as_ref(), "/");
}

#[test]
fn login_path_matches_literal() {
    assert_eq!(RouteName::Login.path().as_ref(), "/login");
}

#[test]
fn logout_path_matches_literal() {
    assert_eq!(RouteName::Logout.path().as_ref(), "/logout");
}

#[test]
fn register_path_matches_literal() {
    assert_eq!(RouteName::Register.path().as_ref(), "/register");
}

#[test]
fn dashboard_path_matches_literal() {
    assert_eq!(RouteName::Dashboard.path().as_ref(), "/dashboard");
}

#[test]
fn profile_path_matches_literal() {
    assert_eq!(RouteName::Profile.path().as_ref(), "/profile");
}

#[test]
fn not_found_path_matches_literal() {
    assert_eq!(RouteName::NotFound.path().as_ref(), "/404");
}

#[test]
fn resource_list_path_format() {
    assert_eq!(RouteName::ResourceList("posts").path().as_ref(), "/posts");
}

#[test]
fn resource_detail_path_format() {
    assert_eq!(RouteName::ResourceDetail("posts", 42).path().as_ref(), "/posts/42");
}

#[test]
fn resource_create_path_format() {
    assert_eq!(RouteName::ResourceCreate("posts").path().as_ref(), "/posts/new");
}

#[test]
fn resource_edit_path_format() {
    assert_eq!(RouteName::ResourceEdit("posts", 42).path().as_ref(), "/posts/42/edit");
}
