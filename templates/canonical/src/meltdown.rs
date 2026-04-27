use std::{collections::HashMap, io::Error as IoError, time::Duration};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use serde_json::json;
use thiserror::Error;

use crate::cata_log;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeltCategory {
    Client,
    Server,
    Transient,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MeltType {
    DatabaseConnection,
    DatabaseError,
    RecordNotFound,
    UniqueViolation,
    ForeignKeyViolation,
    CheckViolation,
    NotNullViolation,

    AuthRejected,
    SessionExpired,
    SessionInvalid,
    SessionMissing,
    InsufficientPermissions,

    ValidationFailed,
    BadRequest,

    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    UnprocessableEntity,
    MethodNotAllowed,
    TooManyRequests,

    FileNotFound,
    FilePermissionDenied,
    FileOperationFailed,

    SerializationFailed,
    DeserializationFailed,
    ConfigurationError,
    EnvironmentError,

    ExternalServiceError,

    Unexpected(String),
}

#[derive(Debug, Error)]
#[error("{details}")]
pub struct MeltDown {
    pub melt_type: MeltType,
    pub details: String,
    pub user_message: Option<String>,
    pub context: Option<HashMap<String, String>>,
    pub retry_after: Option<Duration>,
    pub transient: bool,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl MeltDown {
    pub fn new(melt_type: MeltType, details: impl Into<String>) -> Self {
        let transient = default_transient(&melt_type);
        Self {
            melt_type,
            details: details.into(),
            user_message: None,
            context: None,
            retry_after: None,
            transient,
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_user_message(mut self, message: impl Into<String>) -> Self {
        self.user_message = Some(message.into());
        self
    }

    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    pub fn retry_after(mut self, after: Duration) -> Self {
        self.retry_after = Some(after);
        self
    }

    pub fn mark_transient(mut self, transient: bool) -> Self {
        self.transient = transient;
        self
    }

    pub fn user_message(&self) -> String {
        let Some(msg) = self.user_message.as_ref() else {
            return self.default_user_message();
        };
        msg.clone()
    }

    fn default_user_message(&self) -> String {
        match &self.melt_type {
            MeltType::DatabaseConnection => "Unable to connect to database. Please try again later.".to_string(),
            MeltType::DatabaseError => "A database error occurred. Please try again later.".to_string(),
            MeltType::RecordNotFound => format!("{} not found.", self.details),
            MeltType::UniqueViolation => format!("{} already exists.", self.details),
            MeltType::ForeignKeyViolation => "Referenced data does not exist.".to_string(),
            MeltType::CheckViolation => "Data validation constraints were not met.".to_string(),
            MeltType::NotNullViolation => format!("{} is required.", self.details),

            MeltType::AuthRejected => "Invalid username or password.".to_string(),
            MeltType::SessionExpired => "Your session has expired. Please log in again.".to_string(),
            MeltType::SessionInvalid => "Invalid session. Please log in again.".to_string(),
            MeltType::SessionMissing => "Authentication required.".to_string(),
            MeltType::InsufficientPermissions => "You don't have permission to perform this action.".to_string(),

            MeltType::ValidationFailed => {
                if self.details.is_empty() {
                    "Validation failed".to_string()
                } else {
                    format!("Validation failed: {}", self.details)
                }
            }
            MeltType::BadRequest => format!("Bad request: {}", self.details),

            MeltType::Unauthorized => format!("Unauthorized: {}", self.details),
            MeltType::Forbidden => format!("Forbidden: {}", self.details),
            MeltType::NotFound => format!("{} not found.", self.details),
            MeltType::Conflict => {
                if self.details.is_empty() {
                    "A conflict occurred.".to_string()
                } else {
                    format!("Conflict: {}", self.details)
                }
            }
            MeltType::UnprocessableEntity => {
                if self.details.is_empty() {
                    "The request could not be processed.".to_string()
                } else {
                    format!("Unprocessable: {}", self.details)
                }
            }
            MeltType::MethodNotAllowed => format!("Method {} not allowed.", self.details),
            MeltType::TooManyRequests => "Too many requests. Please slow down.".to_string(),

            MeltType::FileNotFound => format!("File not found: {}", self.details),
            MeltType::FilePermissionDenied => "Permission denied accessing file.".to_string(),
            MeltType::FileOperationFailed => "File operation failed.".to_string(),

            MeltType::SerializationFailed => "Data processing error.".to_string(),
            MeltType::DeserializationFailed => "Data processing error.".to_string(),
            MeltType::ConfigurationError => "Application configuration error.".to_string(),
            MeltType::EnvironmentError => "Environment setup error.".to_string(),

            MeltType::ExternalServiceError => "External service error.".to_string(),

            MeltType::Unexpected(_) => "An unexpected error occurred.".to_string(),
        }
    }

    pub fn log_message(&self) -> String {
        let mut message = format!("[{}] {}", self.melt_type_str(), self.details);

        self.context.as_ref().map(|context| {
            for (key, value) in context {
                message.push_str(&format!(" | {}={}", key, value));
            }
        });

        self.source.as_ref().map(|source| {
            message.push_str(&format!(" | source: {}", source));
        });

        message
    }

    pub fn melt_type_str(&self) -> &'static str {
        match self.melt_type {
            MeltType::DatabaseConnection => "DatabaseConnection",
            MeltType::DatabaseError => "DatabaseError",
            MeltType::RecordNotFound => "RecordNotFound",
            MeltType::UniqueViolation => "UniqueViolation",
            MeltType::ForeignKeyViolation => "ForeignKeyViolation",
            MeltType::CheckViolation => "CheckViolation",
            MeltType::NotNullViolation => "NotNullViolation",
            MeltType::AuthRejected => "AuthRejected",
            MeltType::SessionExpired => "SessionExpired",
            MeltType::SessionInvalid => "SessionInvalid",
            MeltType::SessionMissing => "SessionMissing",
            MeltType::InsufficientPermissions => "InsufficientPermissions",
            MeltType::ValidationFailed => "ValidationFailed",
            MeltType::BadRequest => "BadRequest",
            MeltType::Unauthorized => "Unauthorized",
            MeltType::Forbidden => "Forbidden",
            MeltType::NotFound => "NotFound",
            MeltType::Conflict => "Conflict",
            MeltType::UnprocessableEntity => "UnprocessableEntity",
            MeltType::MethodNotAllowed => "MethodNotAllowed",
            MeltType::TooManyRequests => "TooManyRequests",
            MeltType::FileNotFound => "FileNotFound",
            MeltType::FilePermissionDenied => "FilePermissionDenied",
            MeltType::FileOperationFailed => "FileOperationFailed",
            MeltType::SerializationFailed => "SerializationFailed",
            MeltType::DeserializationFailed => "DeserializationFailed",
            MeltType::ConfigurationError => "ConfigurationError",
            MeltType::EnvironmentError => "EnvironmentError",
            MeltType::ExternalServiceError => "ExternalServiceError",
            MeltType::Unexpected(_) => "Unexpected",
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self.melt_type {
            MeltType::AuthRejected
            | MeltType::SessionExpired
            | MeltType::SessionInvalid
            | MeltType::SessionMissing
            | MeltType::Unauthorized => StatusCode::UNAUTHORIZED,

            MeltType::InsufficientPermissions
            | MeltType::Forbidden
            | MeltType::FilePermissionDenied => StatusCode::FORBIDDEN,

            MeltType::ValidationFailed
            | MeltType::UnprocessableEntity => StatusCode::UNPROCESSABLE_ENTITY,

            MeltType::BadRequest
            | MeltType::CheckViolation
            | MeltType::NotNullViolation => StatusCode::BAD_REQUEST,

            MeltType::NotFound
            | MeltType::RecordNotFound
            | MeltType::FileNotFound => StatusCode::NOT_FOUND,

            MeltType::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,

            MeltType::UniqueViolation
            | MeltType::ForeignKeyViolation
            | MeltType::Conflict => StatusCode::CONFLICT,

            MeltType::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,

            MeltType::ExternalServiceError => StatusCode::SERVICE_UNAVAILABLE,

            MeltType::DatabaseConnection
            | MeltType::DatabaseError
            | MeltType::FileOperationFailed
            | MeltType::SerializationFailed
            | MeltType::DeserializationFailed
            | MeltType::ConfigurationError
            | MeltType::EnvironmentError
            | MeltType::Unexpected(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

fn pick_unique_field(constraint: Option<&str>) -> &'static str {
    let Some(c) = constraint else {
        return "This value";
    };
    if c.contains("email") || c.contains("users_email_key") {
        "Email"
    } else if c.contains("token") || c.contains("sessions_token_key") {
        "Session token"
    } else {
        "This value"
    }
}

fn column_name_or_unknown(name: Option<&str>) -> String {
    let Some(c) = name else {
        return "Unknown field".to_string();
    };
    c.to_string()
}

fn default_transient(melt_type: &MeltType) -> bool {
    matches!(
        melt_type,
        MeltType::DatabaseConnection
            | MeltType::ExternalServiceError
            | MeltType::TooManyRequests
    )
}

impl MeltDown {
    pub fn is_transient(&self) -> bool {
        self.transient
    }

    pub fn is_permanent(&self) -> bool {
        matches!(self.melt_type,
            MeltType::ValidationFailed
            | MeltType::BadRequest
            | MeltType::UnprocessableEntity
            | MeltType::MethodNotAllowed
            | MeltType::AuthRejected
            | MeltType::Unauthorized
            | MeltType::Forbidden
            | MeltType::InsufficientPermissions
            | MeltType::SessionMissing
            | MeltType::SessionInvalid
            | MeltType::SessionExpired
            | MeltType::NotFound
            | MeltType::RecordNotFound
            | MeltType::FileNotFound
            | MeltType::FilePermissionDenied
            | MeltType::Conflict
            | MeltType::UniqueViolation
            | MeltType::ForeignKeyViolation
            | MeltType::CheckViolation
            | MeltType::NotNullViolation
            | MeltType::SerializationFailed
            | MeltType::DeserializationFailed
            | MeltType::ConfigurationError
            | MeltType::EnvironmentError
        )
    }

    pub fn category(&self) -> MeltCategory {
        if self.transient {
            return MeltCategory::Transient;
        }

        let code = self.status_code().as_u16();
        if (400..500).contains(&code) {
            MeltCategory::Client
        } else {
            MeltCategory::Server
        }
    }

    pub fn is(&self, t: MeltType) -> bool {
        match (&self.melt_type, &t) {
            (MeltType::Unexpected(a), MeltType::Unexpected(b)) => a == b,
            (melt_a, melt_b) => melt_a == melt_b,
        }
    }
}

impl From<std::env::VarError> for MeltDown {
    fn from(err: std::env::VarError) -> Self {
        MeltDown::new(MeltType::EnvironmentError, "Environment variable error")
            .with_source(err)
    }
}

impl From<DieselError> for MeltDown {
    fn from(err: DieselError) -> Self {
        match err {
            DieselError::DatabaseError(kind, ref info) => match kind {
                DatabaseErrorKind::UniqueViolation => {
                    let field = pick_unique_field(info.constraint_name());
                    let error = MeltDown::new(MeltType::UniqueViolation, field)
                        .with_context("error_type", "database_unique_violation");
                    let Some(constraint) = info.constraint_name() else {
                        return error;
                    };
                    error.with_context("constraint", constraint)
                }
                DatabaseErrorKind::ForeignKeyViolation => {
                    let error = MeltDown::new(MeltType::ForeignKeyViolation, "Related record not found")
                        .with_context("error_type", "database_foreign_key_violation");
                    let Some(constraint) = info.constraint_name() else {
                        return error;
                    };
                    error.with_context("constraint", constraint)
                }
                DatabaseErrorKind::CheckViolation => {
                    let error = MeltDown::new(MeltType::CheckViolation, "Check constraint failed");
                    let Some(constraint) = info.constraint_name() else {
                        return error;
                    };
                    error.with_context("constraint", constraint)
                }
                DatabaseErrorKind::NotNullViolation => {
                    let column = column_name_or_unknown(info.column_name());
                    let error = MeltDown::new(MeltType::NotNullViolation, column);
                    let Some(table) = info.table_name() else {
                        return error;
                    };
                    error.with_context("table", table)
                }
                other_kind => MeltDown::new(MeltType::DatabaseError, format!("Database error: {:?}", other_kind)),
            },
            DieselError::NotFound => MeltDown::new(MeltType::RecordNotFound, "Record"),
            DieselError::RollbackTransaction => MeltDown::new(MeltType::DatabaseError, "Transaction rolled back"),
            DieselError::AlreadyInTransaction => MeltDown::new(MeltType::DatabaseError, "Already in transaction"),
            DieselError::QueryBuilderError(e) => MeltDown::new(MeltType::DatabaseError, format!("Query builder error: {}", e)),
            DieselError::DeserializationError(e) => MeltDown::new(MeltType::DeserializationFailed, format!("Failed to deserialize result: {}", e)),
            DieselError::SerializationError(e) => MeltDown::new(MeltType::SerializationFailed, format!("Failed to serialize data: {}", e)),
            other_err => MeltDown::new(MeltType::DatabaseError, format!("Database error: {:?}", other_err)),
        }
    }
}

impl From<IoError> for MeltDown {
    fn from(err: IoError) -> Self {
        use std::io::ErrorKind;

        let (melt_type, message) = match err.kind() {
            ErrorKind::NotFound => (MeltType::FileNotFound, "File not found"),
            ErrorKind::PermissionDenied => (MeltType::FilePermissionDenied, "File permission denied"),
            other_kind => (MeltType::FileOperationFailed, "File operation failed"),
        };

        MeltDown::new(melt_type, message).with_source(err)
    }
}

impl IntoResponse for MeltDown {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = json!({
            "error": {
                "code": status.as_u16(),
                "type": self.melt_type_str(),
                "message": self.user_message(),
                "context": self.context,
            }
        });

        let mut response = (status, Json(body)).into_response();

        self.retry_after.map(|retry| {
            match retry.as_secs().to_string().parse() {
                Ok(value) => {
                    response.headers_mut().insert("Retry-After", value);
                }
                Err(e) => {
                    cata_log!(Warning, format!("Retry-After header parse failed: {}", e));
                }
            }
        });

        response
    }
}

impl MeltDown {
    pub fn log(&self) {
        match self.status_code().as_u16() {
            400..=499 => cata_log!(Warning, self.log_message()),
            code => cata_log!(Error, format!("status={} {}", code, self.log_message())),
        }
    }
}

impl MeltDown {
    pub fn db_connection(details: impl Into<String>) -> Self {
        Self::new(MeltType::DatabaseConnection, details)
    }

    pub fn record_not_found(entity: impl Into<String>) -> Self {
        Self::new(MeltType::RecordNotFound, entity)
    }

    pub fn unique_violation(field: impl Into<String>) -> Self {
        Self::new(MeltType::UniqueViolation, field)
    }

    pub fn auth_rejected() -> Self {
        Self::new(MeltType::AuthRejected, "Invalid username or password")
    }

    pub fn session_expired() -> Self {
        Self::new(MeltType::SessionExpired, "Session has expired")
    }

    pub fn session_invalid(details: impl Into<String>) -> Self {
        Self::new(MeltType::SessionInvalid, details)
    }

    pub fn session_missing() -> Self {
        Self::new(MeltType::SessionMissing, "Authentication session is missing")
    }

    pub fn insufficient_permissions() -> Self {
        Self::new(MeltType::InsufficientPermissions, "Insufficient permissions for this action")
    }

    pub fn validation_failed(details: impl Into<String>) -> Self {
        Self::new(MeltType::ValidationFailed, details)
    }

    pub fn bad_request(details: impl Into<String>) -> Self {
        Self::new(MeltType::BadRequest, details)
    }

    pub fn too_many_requests(retry_after: Duration) -> Self {
        Self::new(MeltType::TooManyRequests, "Too many requests").retry_after(retry_after)
    }

    pub fn conflict(details: impl Into<String>) -> Self {
        Self::new(MeltType::Conflict, details)
    }

    pub fn unprocessable_entity(details: impl Into<String>) -> Self {
        Self::new(MeltType::UnprocessableEntity, details)
    }

    pub fn not_found(resource: impl Into<String>, id: impl Into<String>) -> Self {
        let resource = resource.into();
        let id = id.into();
        Self::new(MeltType::RecordNotFound, format!("{} not found: {}", resource, id))
            .with_context("resource", resource)
            .with_context("id", id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
