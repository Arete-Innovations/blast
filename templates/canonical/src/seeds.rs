use diesel_async::AsyncPgConnection;

use crate::{
    cata_log,
    meltdown::*,
    models::auth::users,
    services::crypto,
    structs::auth::Role,
};

const ADMIN_EMAIL: &str = "admin@admin.com";
const ADMIN_PASSWORD: &str = "admin";

pub async fn ensure_admin(conn: &mut AsyncPgConnection) -> Result<(), MeltDown> {
    if users::find_by_email(conn, ADMIN_EMAIL).await?.is_some() {
        cata_log!(Debug, format!("admin user already present: {}", ADMIN_EMAIL));
        return Ok(());
    }

    let hash = crypto::hash_password(ADMIN_PASSWORD)?;
    let user = users::insert_new(conn, ADMIN_EMAIL, &hash).await?;
    users::set_role(conn, user.id, Role::Admin).await?;

    cata_log!(Info, format!("seeded admin user: {}", ADMIN_EMAIL));
    Ok(())
}
