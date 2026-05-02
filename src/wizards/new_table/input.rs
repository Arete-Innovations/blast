use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

use crate::state::names::FieldName;

use super::{
    emit,
    state::{ColumnSpec, ColumnsFocus, FormFocus, PreviewFocus, Screen, WizardState},
};

pub enum Step {
    Stay,
    Cancel,
    Commit,
}

pub fn handle(event: &Event, state: &mut WizardState) -> Step {
    let key = match event {
        Event::Key(k) if k.kind == KeyEventKind::Press => k,
        _other => return Step::Stay,
    };

    if key.code == KeyCode::Esc {
        state.cancelled = true;
        return Step::Cancel;
    }
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.cancelled = true;
        return Step::Cancel;
    }

    state.error = None;

    match state.screen {
        Screen::Form => handle_form(event, key, state),
        Screen::Columns => handle_columns(event, key, state),
        Screen::Preview => handle_preview(key, state),
    }
}

fn handle_form(event: &Event, key: &KeyEvent, state: &mut WizardState) -> Step {
    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            state.form_focus = state.form_focus.cycle(true);
            return Step::Stay;
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.form_focus = state.form_focus.cycle(false);
            return Step::Stay;
        }
        _other => {}
    }

    match state.form_focus {
        FormFocus::TableName => {
            state.table_name.handle_event(event);
            Step::Stay
        }
        FormFocus::IdPk => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.id_pk = !state.id_pk;
            }
            Step::Stay
        }
        FormFocus::CreatedAt => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.created_at = !state.created_at;
            }
            Step::Stay
        }
        FormFocus::UpdatedAt => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.updated_at = !state.updated_at;
            }
            Step::Stay
        }
        FormFocus::SoftDelete => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.soft_delete = !state.soft_delete;
            }
            Step::Stay
        }
        FormFocus::GenLevel => {
            match key.code {
                KeyCode::Left => state.cycle_gen_level(false),
                KeyCode::Right | KeyCode::Char(' ') => state.cycle_gen_level(true),
                _other => {}
            }
            Step::Stay
        }
        FormFocus::VerbList => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.verbs.list = !state.verbs.list;
            }
            Step::Stay
        }
        FormFocus::VerbGet => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.verbs.get = !state.verbs.get;
            }
            Step::Stay
        }
        FormFocus::VerbCreate => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.verbs.create = !state.verbs.create;
            }
            Step::Stay
        }
        FormFocus::VerbUpdate => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.verbs.update = !state.verbs.update;
            }
            Step::Stay
        }
        FormFocus::VerbDelete => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.verbs.delete = !state.verbs.delete;
            }
            Step::Stay
        }
        FormFocus::Next => {
            if matches!(key.code, KeyCode::Enter) {
                match emit::validate_form(state) {
                    Ok(_table) => {
                        state.screen = Screen::Columns;
                        state.columns_focus = ColumnsFocus::DraftName;
                    }
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
            Step::Stay
        }
    }
}

fn handle_columns(event: &Event, key: &KeyEvent, state: &mut WizardState) -> Step {
    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            state.columns_focus = state.columns_focus.cycle(true);
            return Step::Stay;
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.columns_focus = state.columns_focus.cycle(false);
            return Step::Stay;
        }
        _other => {}
    }

    match state.columns_focus {
        ColumnsFocus::DraftName => {
            state.draft.name.handle_event(event);
            Step::Stay
        }
        ColumnsFocus::DraftType => {
            match key.code {
                KeyCode::Left => state.cycle_draft_type(false),
                KeyCode::Right | KeyCode::Char(' ') => state.cycle_draft_type(true),
                _other => {}
            }
            Step::Stay
        }
        ColumnsFocus::DraftNotNull => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.draft.not_null = !state.draft.not_null;
            }
            Step::Stay
        }
        ColumnsFocus::DraftPublicVisible => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                state.draft.public_visible = !state.draft.public_visible;
            }
            Step::Stay
        }
        ColumnsFocus::DraftValidator => {
            match key.code {
                KeyCode::Left => state.draft.cycle_validator(false),
                KeyCode::Right | KeyCode::Char(' ') => state.draft.cycle_validator(true),
                _other => {}
            }
            Step::Stay
        }
        ColumnsFocus::AddColumn => {
            if matches!(key.code, KeyCode::Enter) {
                add_draft_column(state);
            }
            Step::Stay
        }
        ColumnsFocus::DeleteLast => {
            if matches!(key.code, KeyCode::Enter) {
                if state.columns.pop().is_none() {
                    state.error = Some("No columns to delete.".to_string());
                }
            }
            Step::Stay
        }
        ColumnsFocus::Back => {
            if matches!(key.code, KeyCode::Enter) {
                state.screen = Screen::Form;
                state.form_focus = FormFocus::Next;
            }
            Step::Stay
        }
        ColumnsFocus::Done => {
            if matches!(key.code, KeyCode::Enter) {
                match emit::validate(state) {
                    Ok(_t) => {
                        state.screen = Screen::Preview;
                        state.preview_focus = PreviewFocus::Commit;
                    }
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
            Step::Stay
        }
    }
}

fn add_draft_column(state: &mut WizardState) {
    let name = state.draft.name.value().trim().to_string();
    if name.is_empty() {
        state.error = Some("Column name is required.".to_string());
        return;
    }
    if let Err(e) = FieldName::try_new(name.clone()) {
        state.error = Some(e.to_string());
        return;
    }
    let ty = match state.current_draft_type() {
        Some(t) => t,
        None => {
            state.error = Some("No column types available — should never happen.".to_string());
            return;
        }
    };
    let validator = state.draft.current_validator();
    let public_visible = if super::state::looks_sensitive(&name) { false } else { state.draft.public_visible };
    state.columns.push(ColumnSpec {
        name: name.clone(),
        ty,
        not_null: state.draft.not_null,
        public_visible,
        validator,
    });
    state.draft = super::state::ColumnDraft::default();
    state.columns_focus = ColumnsFocus::DraftName;
}

fn handle_preview(key: &KeyEvent, state: &mut WizardState) -> Step {
    match key.code {
        KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
            state.preview_focus = state.preview_focus.toggle();
            Step::Stay
        }
        KeyCode::Enter => match state.preview_focus {
            PreviewFocus::Back => {
                state.screen = Screen::Columns;
                state.columns_focus = ColumnsFocus::Done;
                Step::Stay
            }
            PreviewFocus::Commit => Step::Commit,
        },
        _other => Step::Stay,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use tui_input::Input;

    use super::add_draft_column;
    use super::super::state::{ColumnType, WizardState};

    fn fresh_state() -> WizardState {
        WizardState::new(PathBuf::from("/tmp/blast-test"), vec![ColumnType::Text])
    }

    fn type_draft(state: &mut WizardState, name: &str) {
        state.draft.name = Input::default().with_value(name.to_string());
    }

    #[test]
    fn add_draft_column_rejects_rust_keyword() {
        let mut state = fresh_state();
        type_draft(&mut state, "type");
        add_draft_column(&mut state);
        assert!(state.columns.is_empty(), "keyword name should not be pushed");
        let err = state.error.expect("error must be set on keyword reject");
        assert!(err.contains("keyword") || err.contains("reserved"), "error should mention keyword: {err}");
    }

    #[test]
    fn add_draft_column_rejects_capital_starting_name() {
        let mut state = fresh_state();
        type_draft(&mut state, "FirstName");
        add_draft_column(&mut state);
        assert!(state.columns.is_empty(), "non-snake_case must not be pushed");
        assert!(state.error.is_some(), "error must be set on bad ident");
    }

    #[test]
    fn add_draft_column_rejects_empty_name() {
        let mut state = fresh_state();
        type_draft(&mut state, "   ");
        add_draft_column(&mut state);
        assert!(state.columns.is_empty());
        assert_eq!(state.error.as_deref(), Some("Column name is required."));
    }

    #[test]
    fn add_draft_column_forces_password_hash_public_visible_false() {
        let mut state = fresh_state();
        type_draft(&mut state, "password_hash");
        state.draft.public_visible = true;
        add_draft_column(&mut state);
        assert_eq!(state.columns.len(), 1, "clean name should push");
        assert_eq!(state.error, None);
        assert_eq!(
            state.columns[0].public_visible, false,
            "looks_sensitive must override user toggle for password_hash"
        );
    }

    #[test]
    fn add_draft_column_forces_secret_suffix_public_visible_false() {
        let mut state = fresh_state();
        type_draft(&mut state, "api_secret");
        state.draft.public_visible = true;
        add_draft_column(&mut state);
        assert_eq!(state.columns.len(), 1);
        assert_eq!(state.columns[0].public_visible, false, "_secret suffix must hard-force false");
    }

    #[test]
    fn add_draft_column_forces_token_suffix_public_visible_false() {
        let mut state = fresh_state();
        type_draft(&mut state, "session_token");
        state.draft.public_visible = true;
        add_draft_column(&mut state);
        assert_eq!(state.columns.len(), 1);
        assert_eq!(state.columns[0].public_visible, false, "_token suffix must hard-force false");
    }

    #[test]
    fn add_draft_column_forces_key_suffix_public_visible_false() {
        let mut state = fresh_state();
        type_draft(&mut state, "api_key");
        state.draft.public_visible = true;
        add_draft_column(&mut state);
        assert_eq!(state.columns.len(), 1);
        assert_eq!(state.columns[0].public_visible, false, "_key suffix must hard-force false");
    }

    #[test]
    fn add_draft_column_clean_name_honors_user_toggle_true() {
        let mut state = fresh_state();
        type_draft(&mut state, "email");
        state.draft.public_visible = true;
        add_draft_column(&mut state);
        assert_eq!(state.columns.len(), 1);
        assert_eq!(state.columns[0].public_visible, true, "non-sensitive name must pass the toggle through");
    }

    #[test]
    fn add_draft_column_clean_name_default_visible_false() {
        let mut state = fresh_state();
        type_draft(&mut state, "first_name");
        add_draft_column(&mut state);
        assert_eq!(state.columns.len(), 1);
        assert_eq!(state.columns[0].public_visible, false, "ColumnDraft default is opt-in (false)");
    }

    #[test]
    fn add_draft_column_resets_draft_after_push() {
        let mut state = fresh_state();
        type_draft(&mut state, "name");
        state.draft.public_visible = true;
        state.draft.not_null = false;
        add_draft_column(&mut state);
        assert_eq!(state.columns.len(), 1);
        assert_eq!(state.draft.name.value(), "", "draft name must reset");
        assert_eq!(state.draft.public_visible, false, "draft public_visible must reset to default");
        assert_eq!(state.draft.not_null, true, "draft not_null must reset to default");
    }
}
