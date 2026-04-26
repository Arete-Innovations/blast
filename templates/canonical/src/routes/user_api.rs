use axum::{extract::Path, routing::get, Json, Router};
use diesel::sql_types::{Bool, Int8, Integer, Nullable, Text};
use diesel_async::RunQueryDsl;
use serde_json::{json, Value};

use crate::{cata_log, database::db::establish_connection, meltdown::{MeltDown, MeltType}};

#[derive(diesel::QueryableByName, Debug)]
struct UserRow {
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
    #[diesel(sql_type = Int8)]
    created_at: i64,
    #[diesel(sql_type = Int8)]
    updated_at: i64,
}

async fn get_user_handler(Path(user_id): Path<i32>) -> Result<Json<Value>, MeltDown> {

    cata_log!(Info, format!("Fetching user with ID: {}", user_id));

    let mut conn = establish_connection().await?;
    let rows: Vec<UserRow> = diesel::sql_query(
        "SELECT id, username, email, first_name, last_name, role, active, created_at, updated_at FROM users WHERE id = $1 LIMIT 1"
    )
    .bind::<Integer, _>(user_id)
    .load(&mut conn)
    .await
    .map_err(|e| {
        cata_log!(Warning, format!("Failed to fetch user {}: {}", user_id, e));
        MeltDown::from(e).with_context("operation", "get_user_by_id")
    })?;

    match rows.into_iter().next() {
        Some(user) => {
            cata_log!(Info, format!("Successfully fetched user: {}", user.username));
            Ok(Json(json!({
                "status": "success",
                "data": {
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
            })))
        }
        None => Err(MeltDown::new(MeltType::RecordNotFound, format!("user {}", user_id))),
    }
}

async fn list_users_handler() -> Result<Json<Value>, MeltDown> {

    cata_log!(Info, "Listing all users");

    let mut conn = establish_connection().await?;
    let users: Vec<UserRow> = diesel::sql_query(
        "SELECT id, username, email, first_name, last_name, role, active, created_at, updated_at FROM users WHERE active = true ORDER BY id ASC"
    )
    .load(&mut conn)
    .await
    .map_err(|e| {
        cata_log!(Warning, format!("Failed to list users: {}", e));
        MeltDown::from(e).with_context("operation", "list_users")
    })?;

    let user_list: Vec<_> = users
        .iter()
        .map(|user| {
            json!({
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "first_name": user.first_name,
                "last_name": user.last_name,
                "role": user.role,
                "active": user.active,
                "created_at": user.created_at,
                "updated_at": user.updated_at
            })
        })
        .collect();

    cata_log!(Info, format!("Found {} active users", users.len()));
    Ok(Json(json!({
        "status": "success",
        "data": {
            "users": user_list,
            "count": users.len()
        }
    })))
}

pub fn routes() -> Router {
    Router::new().route("/users", get(list_users_handler)).route("/users/:id", get(get_user_handler))
}
