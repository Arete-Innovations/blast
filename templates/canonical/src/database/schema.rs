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

diesel::table! {
    fuses (id) {
        id -> Int8,
        name -> Text,
        flow_name -> Text,
        schedule_kind -> Text,
        schedule_spec -> Text,
        enabled -> Bool,
        last_run_at -> Nullable<Timestamptz>,
        last_run_status -> Nullable<Text>,
        last_error -> Nullable<Text>,
        next_run_at -> Timestamptz,
        run_count -> Int8,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(sessions, users);
