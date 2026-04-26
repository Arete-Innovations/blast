use axum::{extract::Extension, middleware::from_fn, routing::get, Json, Router};
use diesel::sql_types::{Integer, Text};
use diesel_async::RunQueryDsl;
use serde_json::{json, Value};

use crate::{
    database::db::establish_connection,
    middleware::auth_middleware::{admin_auth_middleware, session_auth_middleware, SessionContext},
};

async fn fetch_username(user_id: i32) -> String {
    let Ok(mut conn) = establish_connection().await else { return String::new() };
    let rows: Vec<UsernameRow> = diesel::sql_query("SELECT username FROM users WHERE id = $1 LIMIT 1")
        .bind::<Integer, _>(user_id)
        .load(&mut conn)
        .await
        .unwrap_or_default();
    rows.into_iter().next().map(|r| r.username).unwrap_or_default()
}

#[derive(diesel::QueryableByName, Debug)]
struct UsernameRow {
    #[diesel(sql_type = Text)]
    username: String,
}

async fn user_dashboard(Extension(ctx): Extension<SessionContext>) -> Json<Value> {
    let username = fetch_username(ctx.user_id).await;

    Json(json!({
        "message": "Welcome to your dashboard!",
        "user": {
            "id": ctx.user_id,
            "username": username,
            "role": ctx.role,
        },
        "access_level": "user"
    }))
}

async fn admin_dashboard(Extension(ctx): Extension<SessionContext>) -> Json<Value> {
    let username = fetch_username(ctx.user_id).await;

    Json(json!({
        "message": "Welcome to the admin dashboard!",
        "user": {
            "id": ctx.user_id,
            "username": username,
            "role": ctx.role,
        },
        "access_level": "admin",
        "admin_features": ["user_management", "system_config", "analytics"]
    }))
}

async fn user_profile(Extension(ctx): Extension<SessionContext>) -> Json<Value> {
    use diesel::sql_types::{Bool, Nullable};

    #[derive(diesel::QueryableByName, Debug)]
    struct ProfileRow {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        username: String,
        #[diesel(sql_type = Nullable<Text>)]
        email: Option<String>,
        #[diesel(sql_type = Text)]
        first_name: String,
        #[diesel(sql_type = Text)]
        last_name: String,
        #[diesel(sql_type = Text)]
        role: String,
        #[diesel(sql_type = Bool)]
        active: bool,
        #[diesel(sql_type = diesel::sql_types::Int8)]
        created_at: i64,
        #[diesel(sql_type = diesel::sql_types::Int8)]
        updated_at: i64,
    }

    let Ok(mut conn) = establish_connection().await else {
        return Json(json!({"success": false, "message": "Failed to connect to database"}));
    };

    let rows: Result<Vec<ProfileRow>, _> = diesel::sql_query(
        "SELECT id, username, email, first_name, last_name, role, active, created_at, updated_at FROM users WHERE id = $1 LIMIT 1"
    )
    .bind::<Integer, _>(ctx.user_id)
    .load(&mut conn)
    .await;

    match rows {
        Ok(rows) => match rows.into_iter().next() {
            Some(user) => Json(json!({
                "success": true,
                "user": {
                    "id": user.id,
                    "username": user.username,
                    "email": user.email,
                    "first_name": user.first_name,
                    "last_name": user.last_name,
                    "role": user.role,
                    "active": user.active,
                    "created_at": user.created_at,
                    "updated_at": user.updated_at
                }
            })),
            None => Json(json!({"success": false, "message": "User not found"})),
        },
        Err(_) => Json(json!({"success": false, "message": "Failed to fetch user profile"})),
    }
}

async fn admin_users_list(Extension(ctx): Extension<SessionContext>) -> Json<Value> {
    use diesel::sql_types::{Bool, Nullable, Int8};

    #[derive(diesel::QueryableByName, Debug)]
    struct UserListRow {
        #[diesel(sql_type = Integer)]
        id: i32,
        #[diesel(sql_type = Text)]
        username: String,
        #[diesel(sql_type = Nullable<Text>)]
        email: Option<String>,
        #[diesel(sql_type = Text)]
        role: String,
        #[diesel(sql_type = Bool)]
        active: bool,
        #[diesel(sql_type = Int8)]
        created_at: i64,
    }

    let requester_username = fetch_username(ctx.user_id).await;

    let Ok(mut conn) = establish_connection().await else {
        return Json(json!({"success": false, "message": "Failed to connect to database"}));
    };

    let rows: Result<Vec<UserListRow>, _> = diesel::sql_query(
        "SELECT id, username, email, role, active, created_at FROM users WHERE active = true ORDER BY id ASC"
    )
    .load(&mut conn)
    .await;

    match rows {
        Ok(users) => Json(json!({
            "success": true,
            "total": users.len(),
            "users": users.iter().map(|u| json!({
                "id": u.id,
                "username": u.username,
                "email": u.email,
                "role": u.role,
                "active": u.active,
                "created_at": u.created_at
            })).collect::<Vec<_>>(),
            "requested_by": {
                "id": ctx.user_id,
                "username": requester_username,
            }
        })),
        Err(e) => Json(json!({
            "success": false,
            "message": format!("Failed to fetch users: {}", e)
        })),
    }
}

pub fn routes() -> Router {
    let user_routes = Router::new()
        .route("/dashboard", get(user_dashboard))
        .route("/profile", get(user_profile))
        .layer(from_fn(session_auth_middleware));

    let admin_routes = Router::new()
        .route("/admin/dashboard", get(admin_dashboard))
        .route("/admin/users", get(admin_users_list))
        .layer(from_fn(admin_auth_middleware));

    user_routes.merge(admin_routes)
}
