use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

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
    if !is_snake_case(&name) {
        state.error = Some(format!("Column '{}' must be snake_case.", name));
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
    state.columns.push(ColumnSpec {
        name: name.clone(),
        ty,
        not_null: state.draft.not_null,
        public_visible: state.draft.public_visible,
        validator,
    });
    state.draft = super::state::ColumnDraft::default();
    if super::state::looks_sensitive(&name) {
        state.draft.public_visible = false;
    }
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

fn is_snake_case(s: &str) -> bool {
    let trimmed = s.trim();
    let first = match trimmed.chars().next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    trimmed.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
