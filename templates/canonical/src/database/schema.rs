// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Int8,
        email -> Text,
        password_hash -> Text,
        role -> Text,
        created_at -> Int8,
        updated_at -> Int8,
        deleted_at -> Nullable<Int8>,
    }
}

diesel::table! {
    sessions (id) {
        id -> Int8,
        user_id -> Int8,
        token -> Text,
        expires_at -> Int8,
        created_at -> Int8,
    }
}

diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(sessions, users);
