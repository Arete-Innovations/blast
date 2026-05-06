//! Per-step draft types: editable user input that converts to typed
//! resource state at commit time. Lives separately from `state.rs` so
//! the wizard's top-level state-machine stays readable.

use std::fmt;

use tui_input::Input;

use crate::state::resource::{AuthMode, CrankPolicy, Verb};

/// Local error type for wizard-side conversions. Wizard validators are
/// not user-facing IO: they convert text inputs into typed values and
/// surface a human-readable diagnostic when conversion fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardError {
    EmptyField { field: &'static str },
    NotANumber { field: &'static str, value: String },
    OutOfRange { field: &'static str, detail: String },
}

impl fmt::Display for WizardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WizardError::EmptyField { field } => write!(f, "{field} is required"),
            WizardError::NotANumber { field, value } => write!(f, "{field} must be a non-negative integer, got: {value}"),
            WizardError::OutOfRange { field, detail } => write!(f, "{field}: {detail}"),
        }
    }
}

pub type WizardResult<T> = std::result::Result<T, WizardError>;

/// One column type the picker exposes. Dynamic list — `Enum(name)` and
/// `Fk(table)` entries are appended at runtime when the project actually
/// declares such targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnType {
    Text,
    Varchar(u32),
    Integer,
    BigInt,
    Boolean,
    Timestamptz,
    Uuid,
    Jsonb,
    Numeric,
    Enum(String),
    Fk(String),
}

impl ColumnType {
    pub fn label(&self) -> String {
        match self {
            ColumnType::Text => "TEXT".to_string(),
            ColumnType::Varchar(n) => format!("VARCHAR({})", n),
            ColumnType::Integer => "INTEGER".to_string(),
            ColumnType::BigInt => "BIGINT".to_string(),
            ColumnType::Boolean => "BOOLEAN".to_string(),
            ColumnType::Timestamptz => "TIMESTAMPTZ".to_string(),
            ColumnType::Uuid => "UUID".to_string(),
            ColumnType::Jsonb => "JSONB".to_string(),
            ColumnType::Numeric => "NUMERIC".to_string(),
            ColumnType::Enum(name) => format!("enum {}", name),
            ColumnType::Fk(table) => format!("FK -> {}(id)", table),
        }
    }

    /// SQL fragment for the column declaration (without name, NULL flag, default).
    pub fn sql_fragment(&self) -> String {
        match self {
            ColumnType::Text => "TEXT".to_string(),
            ColumnType::Varchar(n) => format!("VARCHAR({})", n),
            ColumnType::Integer => "INTEGER".to_string(),
            ColumnType::BigInt => "BIGINT".to_string(),
            ColumnType::Boolean => "BOOLEAN".to_string(),
            ColumnType::Timestamptz => "TIMESTAMPTZ".to_string(),
            ColumnType::Uuid => "UUID".to_string(),
            ColumnType::Jsonb => "JSONB".to_string(),
            ColumnType::Numeric => "NUMERIC".to_string(),
            ColumnType::Enum(name) => name.clone(),
            ColumnType::Fk(table) => format!("BIGINT REFERENCES {}(id)", table),
        }
    }

    /// Default `sql_type` value for the resource RON state file.
    pub fn ron_sql_type(&self) -> &str {
        match self {
            ColumnType::Text => "Text",
            ColumnType::Varchar(_) => "Varchar",
            ColumnType::Integer => "Int4",
            ColumnType::BigInt => "Int8",
            ColumnType::Boolean => "Bool",
            ColumnType::Timestamptz => "Timestamptz",
            ColumnType::Uuid => "Uuid",
            ColumnType::Jsonb => "Jsonb",
            ColumnType::Numeric => "Numeric",
            ColumnType::Enum(_) => "Enum",
            ColumnType::Fk(_) => "Int8",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorChoice {
    None,
    Required,
    Email,
    MaxLen255,
}

impl ValidatorChoice {
    pub const ALL: &'static [ValidatorChoice] = &[ValidatorChoice::None, ValidatorChoice::Required, ValidatorChoice::Email, ValidatorChoice::MaxLen255];

    pub fn label(self) -> &'static str {
        match self {
            ValidatorChoice::None => "None",
            ValidatorChoice::Required => "Required (non-empty)",
            ValidatorChoice::Email => "Email regex",
            ValidatorChoice::MaxLen255 => "MaxLen(255)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnSpec {
    pub name: String,
    pub ty: ColumnType,
    pub not_null: bool,
    pub public_visible: bool,
    pub validator: ValidatorChoice,
}

#[derive(Debug)]
pub struct ColumnDraft {
    pub name: Input,
    pub type_idx: usize,
    pub not_null: bool,
    pub public_visible: bool,
    pub validator_idx: usize,
}

impl Default for ColumnDraft {
    fn default() -> Self {
        Self {
            name: Input::default(),
            type_idx: 0,
            not_null: true,
            public_visible: false,
            validator_idx: 0,
        }
    }
}

impl ColumnDraft {
    pub fn current_validator(&self) -> ValidatorChoice {
        let len = ValidatorChoice::ALL.len();
        ValidatorChoice::ALL[self.validator_idx % len]
    }

    pub fn cycle_validator(&mut self, forward: bool) {
        let len = ValidatorChoice::ALL.len();
        if forward {
            self.validator_idx = (self.validator_idx + 1) % len;
        } else {
            self.validator_idx = (self.validator_idx + len - 1) % len;
        }
    }
}

pub fn looks_sensitive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "password_hash" || lower.ends_with("_secret") || lower.ends_with("_token") || lower.ends_with("_key")
}

/// Auth picker variants surfaced by the per-verb auth step. These map
/// 1:1 to the catalyst `AuthMode` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthChoice {
    Public,
    AuthRequired,
    AdminOnly,
}

impl AuthChoice {
    pub const ALL: &'static [AuthChoice] = &[AuthChoice::Public, AuthChoice::AuthRequired, AuthChoice::AdminOnly];

    pub fn label(self) -> &'static str {
        match self {
            AuthChoice::Public => "Public",
            AuthChoice::AuthRequired => "AuthRequired",
            AuthChoice::AdminOnly => "AdminOnly",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            AuthChoice::Public => "no auth check",
            AuthChoice::AuthRequired => "any signed-in user",
            AuthChoice::AdminOnly => "require UserRole::Admin",
        }
    }

    pub fn to_auth_mode(self) -> AuthMode {
        match self {
            AuthChoice::Public => AuthMode::Public,
            AuthChoice::AuthRequired => AuthMode::AuthRequired,
            AuthChoice::AdminOnly => AuthMode::AdminOnly,
        }
    }

    pub fn from_auth_mode(mode: &AuthMode) -> Self {
        match mode {
            AuthMode::Public => AuthChoice::Public,
            AuthMode::AdminOnly => AuthChoice::AdminOnly,
            AuthMode::AuthRequired | AuthMode::ScopedTo(_) | AuthMode::Roles(_) => AuthChoice::AuthRequired,
        }
    }
}

/// Crank policy variants surfaced by the per-verb retry step.
/// Each variant carries editable numeric inputs; renderer reads them
/// through the `CrankDraft` fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrankChoice {
    None,
    Backoff,
    FixedDelay,
    Immediate,
}

impl CrankChoice {
    pub const ALL: &'static [CrankChoice] = &[CrankChoice::None, CrankChoice::Backoff, CrankChoice::FixedDelay, CrankChoice::Immediate];

    pub fn label(self) -> &'static str {
        match self {
            CrankChoice::None => "None",
            CrankChoice::Backoff => "Backoff (exp delay)",
            CrankChoice::FixedDelay => "FixedDelay (constant)",
            CrankChoice::Immediate => "Immediate (burst)",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            CrankChoice::None => "single attempt, no retry",
            CrankChoice::Backoff => "retry with exponential delay",
            CrankChoice::FixedDelay => "retry with constant delay",
            CrankChoice::Immediate => "retry without delay",
        }
    }
}

/// Editable per-verb retry config. Numeric fields are stored as text
/// inputs so the renderer can show partially-typed digits without
/// premature parsing.
#[derive(Debug, Clone)]
pub struct CrankDraft {
    pub choice: CrankChoice,
    pub max_attempts: Input,
    pub delay_ms: Input,
    pub deadline_ms: Input,
    pub only_transient: bool,
}

impl Default for CrankDraft {
    fn default() -> Self {
        Self {
            choice: CrankChoice::None,
            max_attempts: Input::default().with_value("2".to_string()),
            delay_ms: Input::default().with_value("50".to_string()),
            deadline_ms: Input::default(),
            only_transient: true,
        }
    }
}

impl CrankDraft {
    /// Suggested default for a verb. List/Get get a 2-attempt 50ms-base
    /// backoff; mutating verbs Create/Update/Delete start at no retry.
    pub fn suggested_for(verb: Verb) -> Self {
        match verb {
            Verb::List | Verb::Get => Self {
                choice: CrankChoice::Backoff,
                max_attempts: Input::default().with_value("2".to_string()),
                delay_ms: Input::default().with_value("50".to_string()),
                deadline_ms: Input::default(),
                only_transient: true,
            },
            Verb::Create | Verb::Update | Verb::Delete => Self::default(),
        }
    }

    /// Convert the draft back to a `CrankPolicy`. Returns a typed error
    /// when numeric fields fail to parse or are out of range.
    pub fn to_policy(&self) -> WizardResult<CrankPolicy> {
        match self.choice {
            CrankChoice::None => Ok(CrankPolicy::None),
            CrankChoice::Backoff => {
                let max = parse_attempts(&self.max_attempts)?;
                let base = parse_ms(&self.delay_ms, "base_ms")?;
                let deadline = parse_optional_ms(&self.deadline_ms)?;
                Ok(CrankPolicy::Backoff {
                    max_attempts: max,
                    base_ms: base,
                    only_transient: self.only_transient,
                    deadline_ms: deadline,
                })
            }
            CrankChoice::FixedDelay => {
                let max = parse_attempts(&self.max_attempts)?;
                let delay = parse_ms(&self.delay_ms, "delay_ms")?;
                let deadline = parse_optional_ms(&self.deadline_ms)?;
                Ok(CrankPolicy::FixedDelay {
                    max_attempts: max,
                    delay_ms: delay,
                    only_transient: self.only_transient,
                    deadline_ms: deadline,
                })
            }
            CrankChoice::Immediate => {
                let max = parse_attempts(&self.max_attempts)?;
                let deadline = parse_optional_ms(&self.deadline_ms)?;
                Ok(CrankPolicy::Immediate {
                    max_attempts: max,
                    only_transient: self.only_transient,
                    deadline_ms: deadline,
                })
            }
        }
    }

    pub fn cycle_choice(&mut self, forward: bool) {
        let len = CrankChoice::ALL.len();
        let mut idx = 0;
        for (i, c) in CrankChoice::ALL.iter().enumerate() {
            if *c == self.choice {
                idx = i;
                break;
            }
        }
        let next = if forward { (idx + 1) % len } else { (idx + len - 1) % len };
        self.choice = CrankChoice::ALL[next];
    }
}

fn parse_attempts(input: &Input) -> WizardResult<u32> {
    let raw = input.value().trim();
    if raw.is_empty() {
        return Err(WizardError::EmptyField { field: "max_attempts" });
    }
    let value: u32 = match raw.parse() {
        Ok(v) => v,
        Err(_e) => {
            return Err(WizardError::NotANumber {
                field: "max_attempts",
                value: raw.to_string(),
            })
        }
    };
    if value < 1 {
        return Err(WizardError::OutOfRange {
            field: "max_attempts",
            detail: "must be at least 1".to_string(),
        });
    }
    Ok(value)
}

fn parse_ms(input: &Input, label: &'static str) -> WizardResult<u32> {
    let raw = input.value().trim();
    if raw.is_empty() {
        return Err(WizardError::EmptyField { field: label });
    }
    match raw.parse::<u32>() {
        Ok(v) => Ok(v),
        Err(_e) => Err(WizardError::NotANumber {
            field: label,
            value: raw.to_string(),
        }),
    }
}

fn parse_optional_ms(input: &Input) -> WizardResult<Option<u32>> {
    let raw = input.value().trim();
    if raw.is_empty() {
        return Ok(None);
    }
    match raw.parse::<u32>() {
        Ok(0) => Err(WizardError::OutOfRange {
            field: "deadline_ms",
            detail: "must be greater than 0 when set".to_string(),
        }),
        Ok(v) => Ok(Some(v)),
        Err(_e) => Err(WizardError::NotANumber {
            field: "deadline_ms",
            value: raw.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crank_draft_default_is_none() {
        let d = CrankDraft::default();
        assert_eq!(d.choice, CrankChoice::None);
        assert!(d.only_transient);
    }

    #[test]
    fn crank_draft_suggested_for_list_is_backoff() {
        let d = CrankDraft::suggested_for(Verb::List);
        assert_eq!(d.choice, CrankChoice::Backoff);
        assert_eq!(d.max_attempts.value(), "2");
        assert_eq!(d.delay_ms.value(), "50");
    }

    #[test]
    fn crank_draft_suggested_for_create_is_none() {
        let d = CrankDraft::suggested_for(Verb::Create);
        assert_eq!(d.choice, CrankChoice::None);
    }

    #[test]
    fn crank_draft_to_policy_none() {
        let d = CrankDraft::default();
        assert_eq!(d.to_policy().expect("none policy parses"), CrankPolicy::None);
    }

    #[test]
    fn crank_draft_to_policy_backoff() {
        let d = CrankDraft::suggested_for(Verb::List);
        match d.to_policy().expect("backoff parses") {
            CrankPolicy::Backoff {
                max_attempts,
                base_ms,
                only_transient,
                deadline_ms,
            } => {
                assert_eq!(max_attempts, 2);
                assert_eq!(base_ms, 50);
                assert!(only_transient);
                assert_eq!(deadline_ms, None);
            }
            _other => panic!("expected Backoff variant"),
        }
    }

    #[test]
    fn crank_draft_to_policy_rejects_zero_max_attempts() {
        let mut d = CrankDraft::suggested_for(Verb::List);
        d.max_attempts = Input::default().with_value("0".to_string());
        let err = d.to_policy().expect_err("zero attempts must reject");
        match err {
            WizardError::OutOfRange { field, detail } => {
                assert_eq!(field, "max_attempts");
                assert!(detail.contains("at least 1"));
            }
            other => panic!("expected OutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn crank_draft_to_policy_rejects_negative_via_parse() {
        let mut d = CrankDraft::suggested_for(Verb::List);
        d.max_attempts = Input::default().with_value("-3".to_string());
        let err = d.to_policy().expect_err("negative must reject");
        match err {
            WizardError::NotANumber { field, value } => {
                assert_eq!(field, "max_attempts");
                assert_eq!(value, "-3");
            }
            other => panic!("expected NotANumber, got {other:?}"),
        }
    }

    #[test]
    fn crank_draft_to_policy_rejects_zero_deadline() {
        let mut d = CrankDraft::suggested_for(Verb::List);
        d.deadline_ms = Input::default().with_value("0".to_string());
        let err = d.to_policy().expect_err("zero deadline must reject");
        match err {
            WizardError::OutOfRange { field, detail } => {
                assert_eq!(field, "deadline_ms");
                assert!(detail.contains("greater than 0"));
            }
            other => panic!("expected OutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn auth_choice_round_trips_with_auth_mode() {
        for choice in AuthChoice::ALL {
            let mode = choice.to_auth_mode();
            let restored = AuthChoice::from_auth_mode(&mode);
            assert_eq!(*choice, restored);
        }
    }

    #[test]
    fn looks_sensitive_catches_password_hash_and_suffixes() {
        assert!(looks_sensitive("password_hash"));
        assert!(looks_sensitive("api_secret"));
        assert!(looks_sensitive("session_token"));
        assert!(looks_sensitive("api_key"));
        assert!(!looks_sensitive("email"));
        assert!(!looks_sensitive("first_name"));
    }

    #[test]
    fn column_type_label_round_trips_for_basic_types() {
        assert_eq!(ColumnType::Text.label(), "TEXT");
        assert_eq!(ColumnType::Varchar(64).label(), "VARCHAR(64)");
        assert_eq!(ColumnType::Boolean.label(), "BOOLEAN");
        assert_eq!(ColumnType::Fk("users".to_string()).label(), "FK -> users(id)");
    }
}
