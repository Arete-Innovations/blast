use crate::structs::leptos::BadgeColor;

pub struct AdminUserRow {
    pub name: &'static str,
    pub email: &'static str,
    pub role: &'static str,
    pub role_color: BadgeColor,
    pub status: &'static str,
    pub status_color: BadgeColor,
    pub last_seen_offset_min: i64,
}
