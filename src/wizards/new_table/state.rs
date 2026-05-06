//! Wizard state machine. Tracks the current step, per-verb config maps,
//! and the column-builder draft. Domain types (column shapes, crank
//! drafts, parsers) live in `drafts.rs`.

use std::path::PathBuf;

use indexmap::IndexMap;
use tui_input::Input;

use crate::state::{
    gen_level::GenLevel,
    resource::{AuthMode, Verb},
};

pub use super::drafts::{looks_sensitive, AuthChoice, ColumnDraft, ColumnSpec, ColumnType, CrankChoice, CrankDraft, ValidatorChoice, WizardError, WizardResult};

/// Linear list of wizard steps. Each step focuses on one decision.
/// Steps that don't apply to the current state are skipped at runtime
/// by `WizardState::advance` / `WizardState::retreat`. The final
/// `PreviewCommit` step lets the user jump back to any earlier step
/// before commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepId {
    TableName,
    AutoFeatures,
    GenLevel,
    Verbs,
    PerVerbAuth,
    PerVerbCrank,
    Columns,
    PreviewCommit,
}

impl StepId {
    pub const ALL: &'static [StepId] = &[
        StepId::TableName,
        StepId::AutoFeatures,
        StepId::GenLevel,
        StepId::Verbs,
        StepId::PerVerbAuth,
        StepId::PerVerbCrank,
        StepId::Columns,
        StepId::PreviewCommit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StepId::TableName => "Table name",
            StepId::AutoFeatures => "Auto features",
            StepId::GenLevel => "Codegen depth",
            StepId::Verbs => "Verbs",
            StepId::PerVerbAuth => "Per-verb auth",
            StepId::PerVerbCrank => "Per-verb retry policy",
            StepId::Columns => "Columns",
            StepId::PreviewCommit => "Preview & commit",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            StepId::TableName => "Pick the SQL table name. Use snake_case (`users`, `tweet_likes`). Cannot be a Rust keyword. Required.",
            StepId::AutoFeatures => "Toggle auto-managed columns. `id BIGSERIAL PRIMARY KEY` is the catalyst convention. `created_at`/`updated_at` are epoch BIGINTs with NOW() default. `deleted_at` opts in to soft-delete.",
            StepId::GenLevel => "How far should `blast gen all` propagate? Struct = data shape only. Composables (default) emits validators + REST. Pages emits full CRUD UI. Higher levels imply lower.",
            StepId::Verbs => "Which CRUD verbs should be exposed? List/Get/Create/Update/Delete. Selected verbs flow into the per-verb auth + retry steps.",
            StepId::PerVerbAuth => "Server-side guard for each enabled verb. Public = no check. AuthRequired = any signed-in user. AdminOnly = require UserRole::Admin. Roles(...) = require any of the listed roles.",
            StepId::PerVerbCrank => "Retry policy applied at the flow layer. None = single attempt. Backoff = exponential delay. FixedDelay = constant delay. Immediate = burst retry. Suggested defaults are pre-populated; tune per verb.",
            StepId::Columns => "User-defined columns (in addition to the auto features). Pick a name (snake_case), a type, and toggles for nullable, public-visible, and validator. Press Add to push, Done to advance.",
            StepId::PreviewCommit => "Final review. Generated up.sql / down.sql / RON state file all displayed. Press Enter on Commit to write files and chain `blast migrate` + `blast gen all`. Number keys 1-7 jump back to that step.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Continue,
    Next,
    Back,
    Cancel,
    Commit,
    Jump(StepId),
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
    pub fn cycle(self, focuses: &[FormFocus], forward: bool) -> Self {
        let len = focuses.len();
        if len == 0 {
            return self;
        }
        let mut idx = 0_usize;
        for (i, f) in focuses.iter().enumerate() {
            if *f == self {
                idx = i;
                break;
            }
        }
        let next = if forward { (idx + 1) % len } else { (idx + len - 1) % len };
        focuses[next]
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
pub enum CrankFocus {
    Choice,
    MaxAttempts,
    DelayMs,
    DeadlineMs,
    OnlyTransient,
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

impl VerbToggles {
    pub fn enabled(&self) -> Vec<Verb> {
        let mut out: Vec<Verb> = Vec::new();
        if self.list {
            out.push(Verb::List);
        }
        if self.get {
            out.push(Verb::Get);
        }
        if self.create {
            out.push(Verb::Create);
        }
        if self.update {
            out.push(Verb::Update);
        }
        if self.delete {
            out.push(Verb::Delete);
        }
        out
    }

    pub fn any(&self) -> bool {
        self.list || self.get || self.create || self.update || self.delete
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

#[derive(Debug)]
pub struct WizardState {
    pub project_root: PathBuf,

    /// Index into `StepId::ALL`. The wizard always ticks through the
    /// linear step list, but `is_active(step)` decides whether a step
    /// is reachable given the current state (e.g. PerVerbAuth requires
    /// at least one verb enabled).
    pub step_idx: usize,

    pub form_focus: FormFocus,
    pub columns_focus: ColumnsFocus,

    pub table_name: Input,
    pub id_pk: bool,
    pub created_at: bool,
    pub updated_at: bool,
    pub soft_delete: bool,
    pub gen_level_idx: usize,
    pub verbs: VerbToggles,

    /// Per-verb auth choice, keyed by verb. Pre-populated from
    /// `VerbToggles` when the user reaches the PerVerbAuth step.
    /// Defaults: every verb starts at AuthRequired (matches catalyst's
    /// per-resource convention).
    pub per_verb_auth: IndexMap<Verb, AuthChoice>,
    /// Cursor inside the PerVerbAuth step.
    pub auth_step_verb_idx: usize,

    /// Per-verb retry policy draft. Pre-populated with the suggested
    /// default for each verb when the step is entered.
    pub per_verb_crank: IndexMap<Verb, CrankDraft>,
    /// Cursor inside the PerVerbCrank step.
    pub crank_step_verb_idx: usize,
    /// Focus inside the per-verb form on the Crank step.
    pub crank_focus: CrankFocus,

    pub columns: Vec<ColumnSpec>,
    pub draft: ColumnDraft,
    pub type_palette: Vec<ColumnType>,

    pub error: Option<String>,
    pub cancelled: bool,
}

impl WizardState {
    pub fn new(project_root: PathBuf, type_palette: Vec<ColumnType>) -> Self {
        let toggles = VerbToggles::default();
        let mut per_verb_auth: IndexMap<Verb, AuthChoice> = IndexMap::new();
        let mut per_verb_crank: IndexMap<Verb, CrankDraft> = IndexMap::new();
        for v in toggles.enabled() {
            per_verb_auth.insert(v, AuthChoice::AuthRequired);
            per_verb_crank.insert(v, CrankDraft::suggested_for(v));
        }
        Self {
            project_root,
            step_idx: 0,
            form_focus: FormFocus::TableName,
            columns_focus: ColumnsFocus::DraftName,
            table_name: Input::default(),
            id_pk: true,
            created_at: true,
            updated_at: true,
            soft_delete: false,
            gen_level_idx: GenLevel::ALL.iter().position(|l| *l == GenLevel::Pages).unwrap_or(GenLevel::ALL.len() - 1), // allow: Pages is the locked default — every new resource gets full CRUD pages out of the box
            verbs: toggles,
            per_verb_auth,
            auth_step_verb_idx: 0,
            per_verb_crank,
            crank_step_verb_idx: 0,
            crank_focus: CrankFocus::Choice,
            columns: Vec::new(),
            draft: ColumnDraft::default(),
            type_palette,
            error: None,
            cancelled: false,
        }
    }

    pub fn current_step(&self) -> StepId {
        StepId::ALL[self.step_idx % StepId::ALL.len()]
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

    /// True iff the step is reachable in the current state. Per-verb
    /// steps are skipped when no verbs are enabled or gen_level is too
    /// low. Auto-features step doesn't surface anything if all are off.
    pub fn is_active_step(&self, step: StepId) -> bool {
        match step {
            StepId::PerVerbAuth | StepId::PerVerbCrank => self.verbs.any() && self.gen_level() >= GenLevel::Route,
            StepId::Verbs => self.gen_level() >= GenLevel::Route,
            _other => true,
        }
    }

    /// Advance to the next reachable step. Sets `step_idx` to the
    /// position of the next active step or holds at the final step.
    pub fn advance(&mut self) {
        let total = StepId::ALL.len();
        let mut next = self.step_idx + 1;
        while next < total {
            if self.is_active_step(StepId::ALL[next]) {
                self.step_idx = next;
                self.sync_step_entry();
                return;
            }
            next += 1;
        }
        // No further active step: stay where we are. The renderer will
        // already display the final step.
    }

    /// Move to the previous reachable step.
    pub fn retreat(&mut self) {
        let mut prev = self.step_idx;
        while prev > 0 {
            prev -= 1;
            if self.is_active_step(StepId::ALL[prev]) {
                self.step_idx = prev;
                self.sync_step_entry();
                return;
            }
        }
        self.step_idx = 0;
        self.sync_step_entry();
    }

    /// Jump directly to a target step (used by Preview's "go back to
    /// fix this" affordance). Skips inactive intermediate steps.
    pub fn jump_to(&mut self, target: StepId) {
        for (i, s) in StepId::ALL.iter().enumerate() {
            if *s == target && self.is_active_step(target) {
                self.step_idx = i;
                self.sync_step_entry();
                return;
            }
        }
    }

    /// Re-sync per-step caches when entering a step. The PerVerbAuth /
    /// PerVerbCrank steps need the verb cursor reset, and the per-verb
    /// maps need to be populated for any verb that was just toggled on.
    fn sync_step_entry(&mut self) {
        let step = self.current_step();
        match step {
            StepId::PerVerbAuth => {
                self.auth_step_verb_idx = 0;
                let enabled = self.verbs.enabled();
                for v in &enabled {
                    if !self.per_verb_auth.contains_key(v) {
                        self.per_verb_auth.insert(*v, AuthChoice::AuthRequired);
                    }
                }
                let enabled_set: std::collections::BTreeSet<Verb> = enabled.iter().copied().collect();
                self.per_verb_auth.retain(|v, _| enabled_set.contains(v));
            }
            StepId::PerVerbCrank => {
                self.crank_step_verb_idx = 0;
                self.crank_focus = CrankFocus::Choice;
                let enabled = self.verbs.enabled();
                for v in &enabled {
                    if !self.per_verb_crank.contains_key(v) {
                        self.per_verb_crank.insert(*v, CrankDraft::suggested_for(*v));
                    }
                }
                let enabled_set: std::collections::BTreeSet<Verb> = enabled.iter().copied().collect();
                self.per_verb_crank.retain(|v, _| enabled_set.contains(v));
            }
            StepId::Columns => {
                self.columns_focus = ColumnsFocus::DraftName;
            }
            _other => {}
        }
    }

    pub fn current_auth_verb(&self) -> Option<Verb> {
        let verbs: Vec<Verb> = self.per_verb_auth.keys().copied().collect();
        verbs.get(self.auth_step_verb_idx).copied()
    }

    pub fn current_crank_verb(&self) -> Option<Verb> {
        let verbs: Vec<Verb> = self.per_verb_crank.keys().copied().collect();
        verbs.get(self.crank_step_verb_idx).copied()
    }

    pub fn cycle_auth_verb(&mut self, forward: bool) {
        let len = self.per_verb_auth.len();
        if len == 0 {
            return;
        }
        self.auth_step_verb_idx = if forward { (self.auth_step_verb_idx + 1) % len } else { (self.auth_step_verb_idx + len - 1) % len };
    }

    pub fn cycle_crank_verb(&mut self, forward: bool) {
        let len = self.per_verb_crank.len();
        if len == 0 {
            return;
        }
        self.crank_step_verb_idx = if forward { (self.crank_step_verb_idx + 1) % len } else { (self.crank_step_verb_idx + len - 1) % len };
    }

    /// Visible (active) step count, used for "step N/M" rendering.
    pub fn step_progress(&self) -> (usize, usize) {
        let total = StepId::ALL.iter().filter(|s| self.is_active_step(**s)).count();
        let active_so_far = StepId::ALL.iter().take(self.step_idx + 1).filter(|s| self.is_active_step(**s)).count();
        (active_so_far, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fresh() -> WizardState {
        WizardState::new(PathBuf::from("/tmp/test"), vec![ColumnType::Text])
    }

    #[test]
    fn first_step_is_table_name() {
        let s = fresh();
        assert_eq!(s.current_step(), StepId::TableName);
    }

    #[test]
    fn advance_walks_table_to_auto_to_gen_level() {
        let mut s = fresh();
        assert_eq!(s.current_step(), StepId::TableName);
        s.advance();
        assert_eq!(s.current_step(), StepId::AutoFeatures);
        s.advance();
        assert_eq!(s.current_step(), StepId::GenLevel);
    }

    #[test]
    fn advance_skips_per_verb_steps_when_no_verbs_enabled() {
        let mut s = fresh();
        s.verbs = VerbToggles {
            list: false,
            get: false,
            create: false,
            update: false,
            delete: false,
        };
        // jump close to the verb step
        s.step_idx = StepId::ALL.iter().position(|x| *x == StepId::Verbs).unwrap_or(0);
        s.advance();
        assert_eq!(s.current_step(), StepId::Columns, "skip per-verb when verbs empty");
    }

    #[test]
    fn advance_skips_per_verb_when_gen_level_too_low() {
        let mut s = fresh();
        s.gen_level_idx = GenLevel::ALL.iter().position(|l| *l == GenLevel::Struct).unwrap_or(0);
        // walk: TableName -> AutoFeatures -> GenLevel -> (skip Verbs)/(skip per-verb) -> Columns
        s.advance();
        s.advance();
        s.advance();
        assert_eq!(s.current_step(), StepId::Columns);
    }

    #[test]
    fn retreat_walks_back_through_active_steps() {
        let mut s = fresh();
        s.advance();
        s.advance();
        assert_eq!(s.current_step(), StepId::GenLevel);
        s.retreat();
        assert_eq!(s.current_step(), StepId::AutoFeatures);
        s.retreat();
        assert_eq!(s.current_step(), StepId::TableName);
    }

    #[test]
    fn jump_to_skips_inactive() {
        let mut s = fresh();
        s.gen_level_idx = GenLevel::ALL.iter().position(|l| *l == GenLevel::Struct).unwrap_or(0);
        s.jump_to(StepId::PerVerbAuth);
        // Step is inactive at gen_level Struct, so jump should be a no-op.
        assert_eq!(s.current_step(), StepId::TableName);
    }

    #[test]
    fn step_progress_counts_only_active_steps() {
        let mut s = fresh();
        s.gen_level_idx = GenLevel::ALL.iter().position(|l| *l == GenLevel::Struct).unwrap_or(0);
        let (cur, total) = s.step_progress();
        assert_eq!(cur, 1, "TableName is step 1");
        assert!(total < StepId::ALL.len(), "fewer active steps when gen_level low: {total}");
    }

    #[test]
    fn step_id_help_non_empty_for_every_variant() {
        for s in StepId::ALL {
            assert!(!s.help().is_empty(), "missing help: {:?}", s);
            assert!(!s.label().is_empty(), "missing label: {:?}", s);
        }
    }

    #[test]
    fn sync_step_entry_repopulates_per_verb_maps() {
        let mut s = fresh();
        s.verbs = VerbToggles {
            list: true,
            get: false,
            create: true,
            update: false,
            delete: false,
        };
        s.jump_to(StepId::PerVerbAuth);
        // Only enabled verbs should remain in the map
        let keys: Vec<Verb> = s.per_verb_auth.keys().copied().collect();
        assert!(keys.contains(&Verb::List));
        assert!(keys.contains(&Verb::Create));
        assert!(!keys.contains(&Verb::Get));
        assert!(!keys.contains(&Verb::Update));
        assert!(!keys.contains(&Verb::Delete));
    }
}
