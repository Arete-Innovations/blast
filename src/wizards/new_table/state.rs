use std::path::PathBuf;

use tui_input::Input;

use crate::state::gen_level::GenLevel;

/// Top-level wizard screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Form,
    Columns,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFocus {
    TableName,
    IdPk,
    CreatedAt,
    UpdatedAt,
    SoftDelete,
    GenLevel,
    VerbList,
    VerbGet,
    VerbCreate,
    VerbUpdate,
    VerbDelete,
    Next,
}

impl FormFocus {
    pub const ALL: &'static [FormFocus] = &[
        FormFocus::TableName,
        FormFocus::IdPk,
        FormFocus::CreatedAt,
        FormFocus::UpdatedAt,
        FormFocus::SoftDelete,
        FormFocus::GenLevel,
        FormFocus::VerbList,
        FormFocus::VerbGet,
        FormFocus::VerbCreate,
        FormFocus::VerbUpdate,
        FormFocus::VerbDelete,
        FormFocus::Next,
    ];

    pub fn cycle(self, forward: bool) -> Self {
        let len = Self::ALL.len();
        let mut idx = 0_usize;
        for (i, f) in Self::ALL.iter().enumerate() {
            if *f == self {
                idx = i;
                break;
            }
        }
        let next = if forward { (idx + 1) % len } else { (idx + len - 1) % len };
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnsFocus {
    DraftName,
    DraftType,
    DraftNotNull,
    DraftPublicVisible,
    DraftValidator,
    AddColumn,
    DeleteLast,
    Back,
    Done,
}

impl ColumnsFocus {
    pub const ALL: &'static [ColumnsFocus] = &[
        ColumnsFocus::DraftName,
        ColumnsFocus::DraftType,
        ColumnsFocus::DraftNotNull,
        ColumnsFocus::DraftPublicVisible,
        ColumnsFocus::DraftValidator,
        ColumnsFocus::AddColumn,
        ColumnsFocus::DeleteLast,
        ColumnsFocus::Back,
        ColumnsFocus::Done,
    ];

    pub fn cycle(self, forward: bool) -> Self {
        let len = Self::ALL.len();
        let mut idx = 0_usize;
        for (i, f) in Self::ALL.iter().enumerate() {
            if *f == self {
                idx = i;
                break;
            }
        }
        let next = if forward { (idx + 1) % len } else { (idx + len - 1) % len };
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewFocus {
    Back,
    Commit,
}

impl PreviewFocus {
    pub fn toggle(self) -> Self {
        match self {
            PreviewFocus::Back => PreviewFocus::Commit,
            PreviewFocus::Commit => PreviewFocus::Back,
        }
    }
}

/// Verbs the wizard can toggle. CRUD baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerbToggles {
    pub list: bool,
    pub get: bool,
    pub create: bool,
    pub update: bool,
    pub delete: bool,
}

impl Default for VerbToggles {
    fn default() -> Self {
        Self {
            list: true,
            get: true,
            create: true,
            update: true,
            delete: true,
        }
    }
}

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
            ColumnType::Fk(table) => format!("FK → {}(id)", table),
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

#[derive(Debug)]
pub struct WizardState {
    pub project_root: PathBuf,

    pub screen: Screen,

    pub form_focus: FormFocus,
    pub columns_focus: ColumnsFocus,
    pub preview_focus: PreviewFocus,

    pub table_name: Input,
    pub id_pk: bool,
    pub created_at: bool,
    pub updated_at: bool,
    pub soft_delete: bool,
    pub gen_level_idx: usize,
    pub verbs: VerbToggles,

    pub columns: Vec<ColumnSpec>,
    pub draft: ColumnDraft,

    pub type_palette: Vec<ColumnType>,

    pub error: Option<String>,
    pub cancelled: bool,
}

impl WizardState {
    pub fn new(project_root: PathBuf, type_palette: Vec<ColumnType>) -> Self {
        Self {
            project_root,
            screen: Screen::Form,
            form_focus: FormFocus::TableName,
            columns_focus: ColumnsFocus::DraftName,
            preview_focus: PreviewFocus::Commit,
            table_name: Input::default(),
            id_pk: true,
            created_at: true,
            updated_at: true,
            soft_delete: false,
            gen_level_idx: GenLevel::ALL.iter().position(|l| *l == GenLevel::Pages).unwrap_or(GenLevel::ALL.len() - 1), // allow: Pages is the locked default — every new resource gets full CRUD pages out of the box
            verbs: VerbToggles::default(),
            columns: Vec::new(),
            draft: ColumnDraft::default(),
            type_palette,
            error: None,
            cancelled: false,
        }
    }

    pub fn gen_level(&self) -> GenLevel {
        let len = GenLevel::ALL.len();
        let idx = self.gen_level_idx % len;
        GenLevel::ALL[idx]
    }

    pub fn current_draft_type(&self) -> Option<ColumnType> {
        let len = self.type_palette.len();
        if len == 0 {
            return None;
        }
        let idx = self.draft.type_idx % len;
        Some(self.type_palette[idx].clone())
    }

    pub fn cycle_gen_level(&mut self, forward: bool) {
        let len = GenLevel::ALL.len();
        if forward {
            self.gen_level_idx = (self.gen_level_idx + 1) % len;
        } else {
            self.gen_level_idx = (self.gen_level_idx + len - 1) % len;
        }
    }

    pub fn cycle_draft_type(&mut self, forward: bool) {
        let len = self.type_palette.len();
        if len == 0 {
            return;
        }
        if forward {
            self.draft.type_idx = (self.draft.type_idx + 1) % len;
        } else {
            self.draft.type_idx = (self.draft.type_idx + len - 1) % len;
        }
    }
}

/// Outcome handed back to the caller after the TUI exits.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub cancelled: bool,
    pub table_name: String,
    pub up_sql_path: PathBuf,
    pub down_sql_path: PathBuf,
    pub ron_path: PathBuf,
}
