use std::io;

#[derive(Debug, thiserror::Error)]
pub enum BlastError {
    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("postgres connection: {0}")]
    PgConnection(#[from] diesel::ConnectionError),

    #[error("query: {0}")]
    Query(#[from] diesel::result::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("toml parse: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("toml ser: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("env: {0}")]
    Env(#[from] std::env::VarError),

    #[error("strip prefix: {0}")]
    StripPrefix(#[from] std::path::StripPrefixError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("config: {0}")]
    Config(String),

    #[error("subprocess `{cmd}` failed: {detail}")]
    Subprocess { cmd: String, detail: String },

    #[error("missing dependency: {0}")]
    MissingDep(String),

    #[error("project: {0}")]
    Project(String),

    #[error("fuse: {0}")]
    Fuse(String),

    #[error("dashboard: {0}")]
    Dashboard(String),
}

pub type BlastResult<T> = Result<T, BlastError>;
