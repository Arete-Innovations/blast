use canonical::meltdown::{MeltDown, MeltType};

#[test]
fn not_found_melt_type_is_record_not_found() {
    let err = MeltDown::not_found("user", "42");
    assert_eq!(err.melt_type, MeltType::RecordNotFound);
}

#[test]
fn not_found_status_code_is_404() {
    let err = MeltDown::not_found("user", "42");
    assert_eq!(err.status_code(), axum::http::StatusCode::NOT_FOUND);
}

#[test]
fn not_found_context_has_resource() {
    let err = MeltDown::not_found("user", "42");
    let ctx = err.context.as_ref().expect("context must be set");
    assert_eq!(ctx.get("resource").map(String::as_str), Some("user"));
}

#[test]
fn not_found_context_has_id() {
    let err = MeltDown::not_found("user", "42");
    let ctx = err.context.as_ref().expect("context must be set");
    assert_eq!(ctx.get("id").map(String::as_str), Some("42"));
}

#[test]
fn not_found_user_message_contains_resource_and_id() {
    let err = MeltDown::not_found("user", "42");
    let msg = err.user_message();
    assert!(msg.contains("user"), "user_message should contain resource: {}", msg);
    assert!(msg.contains("42"), "user_message should contain id: {}", msg);
}
