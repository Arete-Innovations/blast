use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::state::resource::Verb;

use super::{
    emit,
    state::{AuthChoice, ColumnsFocus, CrankChoice, CrankFocus, FormFocus, StepId, WizardState},
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const ERROR: Color = Color::LightRed;

pub fn draw(frame: &mut Frame<'_>, state: &mut WizardState) {
    let area = frame.area();
    // Title bar (3) | content + help split (min 15) | hint bar (3)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(15), Constraint::Length(3)])
        .split(area);

    let step = state.current_step();
    let (cur, total) = state.step_progress();
    let title = format!("blast - new table  {}", current_table_label(state));
    let subtitle = format!("step {}/{} - {}", cur, total, step.label());
    frame.render_widget(title_block(&title, &subtitle), chunks[0]);

    // 70/30 split for body / help.
    let body_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[1]);

    match step {
        StepId::TableName => draw_table_name(frame, body_split[0], state),
        StepId::AutoFeatures => draw_auto_features(frame, body_split[0], state),
        StepId::GenLevel => draw_gen_level(frame, body_split[0], state),
        StepId::Verbs => draw_verbs(frame, body_split[0], state),
        StepId::PerVerbAuth => draw_per_verb_auth(frame, body_split[0], state),
        StepId::PerVerbCrank => draw_per_verb_crank(frame, body_split[0], state),
        StepId::Columns => draw_columns(frame, body_split[0], state),
        StepId::PreviewCommit => draw_preview(frame, body_split[0], state),
    }

    let help_text = step.help();
    frame.render_widget(help_panel(help_text), body_split[1]);

    let hint = match step {
        StepId::TableName => "Enter advance  Esc cancel",
        StepId::AutoFeatures | StepId::Verbs => "Tab focus  Space toggle  Enter advance  PgUp back  Esc cancel",
        StepId::GenLevel => "<- / -> cycle  Enter advance  PgUp back  Esc cancel",
        StepId::PerVerbAuth => "<- / -> cycle auth  Tab next verb  Enter advance  PgUp back  Esc cancel",
        StepId::PerVerbCrank => "Tab field  <- / -> cycle/edit  Tab next verb  Enter advance  PgUp back  Esc cancel",
        StepId::Columns => "Tab focus  Space toggle  <- / -> cycle types  Enter add/done  PgUp back  Esc cancel",
        StepId::PreviewCommit => "Up/Down navigate  Enter jump-back / commit  PgUp prev step  Esc cancel",
    };
    frame.render_widget(hint_bar(state, hint), chunks[2]);
}

fn current_table_label(state: &WizardState) -> String {
    let name = state.table_name.value().trim();
    if name.is_empty() {
        String::from("(unnamed)")
    } else {
        format!("({})", name)
    }
}

// --- step renderers --------------------------------------------------------

fn draw_table_name(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);
    frame.render_widget(text_input_box(state.table_name.value(), true, "Table name (snake_case)"), chunks[0]);
    let cx = chunks[0].x + 1 + state.table_name.visual_cursor() as u16;
    let cy = chunks[0].y + 1;
    frame.set_cursor_position((cx.min(chunks[0].x + chunks[0].width.saturating_sub(2)), cy));
}

fn draw_auto_features(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let focuses: &[FormFocus] = &[FormFocus::IdPk, FormFocus::CreatedAt, FormFocus::UpdatedAt, FormFocus::SoftDelete, FormFocus::Next];
    if !focuses.contains(&state.form_focus) {
        state.form_focus = focuses[0];
    }
    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);
    frame.render_widget(section_label("Auto features"), chunks[0]);
    frame.render_widget(checkbox_line(state.id_pk, "id BIGSERIAL PRIMARY KEY", state.form_focus == FormFocus::IdPk), chunks[1]);
    frame.render_widget(checkbox_line(state.created_at, "created_at BIGINT (epoch)", state.form_focus == FormFocus::CreatedAt), chunks[2]);
    frame.render_widget(checkbox_line(state.updated_at, "updated_at BIGINT (epoch)", state.form_focus == FormFocus::UpdatedAt), chunks[3]);
    frame.render_widget(checkbox_line(state.soft_delete, "deleted_at BIGINT NULL  (soft-delete)", state.form_focus == FormFocus::SoftDelete), chunks[4]);
    frame.render_widget(button_line("[ Next -> ]", state.form_focus == FormFocus::Next), chunks[6]);
}

fn draw_gen_level(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    frame.render_widget(section_label("Codegen depth"), chunks[0]);
    let level = state.gen_level();
    frame.render_widget(picker_line(level.label(), level.description(), true), chunks[1]);
}

fn draw_verbs(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let focuses: &[FormFocus] = &[FormFocus::VerbList, FormFocus::VerbGet, FormFocus::VerbCreate, FormFocus::VerbUpdate, FormFocus::VerbDelete, FormFocus::Next];
    if !focuses.contains(&state.form_focus) {
        state.form_focus = focuses[0];
    }
    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);
    frame.render_widget(section_label("Verbs (Space toggles)"), chunks[0]);
    frame.render_widget(checkbox_line(state.verbs.list, "List   (GET /<table>)", state.form_focus == FormFocus::VerbList), chunks[1]);
    frame.render_widget(checkbox_line(state.verbs.get, "Get    (GET /<table>/:id)", state.form_focus == FormFocus::VerbGet), chunks[2]);
    frame.render_widget(checkbox_line(state.verbs.create, "Create (POST /<table>)", state.form_focus == FormFocus::VerbCreate), chunks[3]);
    frame.render_widget(checkbox_line(state.verbs.update, "Update (PATCH /<table>/:id)", state.form_focus == FormFocus::VerbUpdate), chunks[4]);
    frame.render_widget(checkbox_line(state.verbs.delete, "Delete (DELETE /<table>/:id)", state.form_focus == FormFocus::VerbDelete), chunks[5]);
    frame.render_widget(button_line("[ Next -> ]", state.form_focus == FormFocus::Next), chunks[7]);
}

fn draw_per_verb_auth(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(8), Constraint::Min(0)])
        .split(inner);
    frame.render_widget(section_label("Per-verb auth (Tab cycles verb, <- / -> cycles auth)"), chunks[0]);

    let verbs: Vec<Verb> = state.per_verb_auth.keys().copied().collect();
    let items: Vec<ListItem> = verbs
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let choice = match state.per_verb_auth.get(v) {
                Some(c) => *c,
                None => AuthChoice::AuthRequired,
            };
            let prefix = if i == state.auth_step_verb_idx { "> " } else { "  " };
            let line = format!("{prefix}{}: {} ({})", verb_label(*v), choice.label(), choice.description());
            let style = if i == state.auth_step_verb_idx {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Verbs"));
    frame.render_widget(list, chunks[1]);
}

fn draw_per_verb_crank(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(7), Constraint::Min(0)])
        .split(inner);
    frame.render_widget(section_label("Per-verb retry policy (Tab cycles fields, <- / -> cycles values)"), chunks[0]);

    let verb = match state.current_crank_verb() {
        Some(v) => v,
        None => {
            frame.render_widget(Paragraph::new("(no verbs enabled - go back)").block(Block::default().borders(Borders::ALL)), chunks[1]);
            return;
        }
    };
    let draft = match state.per_verb_crank.get(&verb) {
        Some(d) => d.clone(),
        None => return,
    };

    let header = format!("Verb: {}  ({}/{})", verb_label(verb), state.crank_step_verb_idx + 1, state.per_verb_crank.len());
    let inner_block = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(chunks[1]);

    frame.render_widget(Paragraph::new(Line::from(Span::styled(header, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)))), inner_block[0]);
    frame.render_widget(picker_line(draft.choice.label(), draft.choice.description(), state.crank_focus == CrankFocus::Choice), inner_block[1]);

    if matches!(draft.choice, CrankChoice::None) {
        let note = "no further inputs - single attempt, no retry";
        frame.render_widget(Paragraph::new(Line::from(Span::styled(note, Style::default().fg(MUTED)))), inner_block[2]);
        return;
    }
    frame.render_widget(text_input_inline("max_attempts", draft.max_attempts.value(), state.crank_focus == CrankFocus::MaxAttempts), inner_block[2]);
    let delay_label = match draft.choice {
        CrankChoice::Backoff => "base_ms",
        CrankChoice::FixedDelay => "delay_ms",
        CrankChoice::Immediate => "(no delay)",
        CrankChoice::None => "(unused)",
    };
    if matches!(draft.choice, CrankChoice::Backoff | CrankChoice::FixedDelay) {
        frame.render_widget(text_input_inline(delay_label, draft.delay_ms.value(), state.crank_focus == CrankFocus::DelayMs), inner_block[3]);
    } else {
        frame.render_widget(Paragraph::new(Line::from(Span::styled(delay_label, Style::default().fg(MUTED)))), inner_block[3]);
    }
    frame.render_widget(text_input_inline("deadline_ms (blank = none)", draft.deadline_ms.value(), state.crank_focus == CrankFocus::DeadlineMs), inner_block[4]);
    frame.render_widget(checkbox_line(draft.only_transient, "only_transient (skip permanent errors)", state.crank_focus == CrankFocus::OnlyTransient), inner_block[5]);
}

fn draw_columns(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Min(12)])
        .split(inner);

    let mut existing: Vec<ListItem> = Vec::new();
    if state.id_pk {
        existing.push(ListItem::new(Line::from(vec![Span::styled("[auto] ", Style::default().fg(MUTED)), Span::raw("id BIGSERIAL PRIMARY KEY")])));
    }
    for col in &state.columns {
        let null_tag = if col.not_null { " NOT NULL" } else { "" };
        let visibility = if col.public_visible { "" } else { "  [admin-only]" };
        let validator_tag = match col.validator {
            super::state::ValidatorChoice::None => String::new(),
            other => format!("  [{}]", other.label()),
        };
        existing.push(ListItem::new(format!("{} {}{}{}{}", col.name, col.ty.label(), null_tag, visibility, validator_tag)));
    }
    if state.created_at {
        existing.push(ListItem::new(Line::from(vec![
            Span::styled("[auto] ", Style::default().fg(MUTED)),
            Span::raw("created_at BIGINT NOT NULL DEFAULT epoch"),
        ])));
    }
    if state.updated_at {
        existing.push(ListItem::new(Line::from(vec![
            Span::styled("[auto] ", Style::default().fg(MUTED)),
            Span::raw("updated_at BIGINT NOT NULL DEFAULT epoch"),
        ])));
    }
    if state.soft_delete {
        existing.push(ListItem::new(Line::from(vec![Span::styled("[auto] ", Style::default().fg(MUTED)), Span::raw("deleted_at BIGINT NULL")])));
    }
    let list = List::new(existing).block(Block::default().borders(Borders::ALL).title("Columns"));
    frame.render_widget(list, chunks[0]);

    let draft_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(chunks[1].inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 }));

    frame.render_widget(text_input_inline("name", state.draft.name.value(), state.columns_focus == ColumnsFocus::DraftName), draft_chunks[0]);

    let type_label = match state.current_draft_type() {
        Some(ty) => ty.label(),
        None => "(no types available)".to_string(),
    };
    frame.render_widget(picker_line(&type_label, "Type - <- / -> cycles", state.columns_focus == ColumnsFocus::DraftType), draft_chunks[1]);
    frame.render_widget(checkbox_line(state.draft.not_null, "NOT NULL", state.columns_focus == ColumnsFocus::DraftNotNull), draft_chunks[2]);
    frame.render_widget(
        checkbox_line(
            state.draft.public_visible,
            "Public-visible (false = hidden from /api responses)",
            state.columns_focus == ColumnsFocus::DraftPublicVisible,
        ),
        draft_chunks[3],
    );
    frame.render_widget(
        picker_line(state.draft.current_validator().label(), "Validator - <- / -> cycles", state.columns_focus == ColumnsFocus::DraftValidator),
        draft_chunks[4],
    );

    frame.render_widget(button_line("[ + Add column ]", state.columns_focus == ColumnsFocus::AddColumn), draft_chunks[6]);
    frame.render_widget(button_line("[ - Delete last column ]", state.columns_focus == ColumnsFocus::DeleteLast), draft_chunks[7]);

    let nav_buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(draft_chunks[9]);
    frame.render_widget(button_line("[ <- Back ]", state.columns_focus == ColumnsFocus::Back), nav_buttons[0]);
    frame.render_widget(button_line("[ Done -> Preview ]", state.columns_focus == ColumnsFocus::Done), nav_buttons[1]);

    let cursor = match state.columns_focus {
        ColumnsFocus::DraftName => {
            let rect = draft_chunks[0];
            let prefix_len: u16 = 7; // " name: "
            let cx = (rect.x + prefix_len + state.draft.name.visual_cursor() as u16).min(rect.x + rect.width.saturating_sub(1));
            Some((cx, rect.y))
        }
        _other => None,
    };
    match cursor {
        Some((cx, cy)) => frame.set_cursor_position((cx, cy)),
        None => {}
    }
}

fn draw_preview(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let arts = match emit::build_safely(state) {
        Ok(a) => a,
        Err(e) => {
            let para = Paragraph::new(format!("preview unavailable: {}", e))
                .block(Block::default().borders(Borders::ALL).title("preview error"))
                .wrap(Wrap { trim: false });
            frame.render_widget(para, area);
            return;
        }
    };

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(15), Constraint::Percentage(35), Constraint::Length(3)])
        .split(area);

    frame.render_widget(Paragraph::new(arts.up_sql).block(Block::default().borders(Borders::ALL).title("up.sql")).wrap(Wrap { trim: false }), body[0]);
    frame.render_widget(Paragraph::new(arts.down_sql).block(Block::default().borders(Borders::ALL).title("down.sql")).wrap(Wrap { trim: false }), body[1]);
    frame.render_widget(
        Paragraph::new(arts.resource_ron)
            .block(Block::default().borders(Borders::ALL).title(format!("storage/blast/state/resources/{}.ron", state.table_name.value())))
            .wrap(Wrap { trim: false }),
        body[2],
    );

    frame.render_widget(button_line("[ Commit + Run Pipeline -> ]", true), body[3]);
}

// --- helpers ---------------------------------------------------------------

fn title_block(title: &str, subtitle: &str) -> Paragraph<'static> {
    let line = Line::from(vec![
        Span::styled(title.to_string(), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(subtitle.to_string(), Style::default().fg(MUTED)),
    ]);
    Paragraph::new(line).block(Block::default().borders(Borders::ALL))
}

fn section_label(text: &str) -> Paragraph<'static> {
    Paragraph::new(Span::styled(text.to_string(), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)))
}

fn text_input_box(value: &str, focused: bool, label: &str) -> Paragraph<'static> {
    let style = if focused { Style::default().fg(ACCENT) } else { Style::default() };
    Paragraph::new(value.to_string()).block(Block::default().borders(Borders::ALL).title(label.to_string()).border_style(style))
}

fn text_input_inline(label: &str, value: &str, focused: bool) -> Paragraph<'static> {
    let label_style = if focused { Style::default().fg(ACCENT).add_modifier(Modifier::BOLD) } else { Style::default().fg(MUTED) };
    let value_style = if focused { Style::default().fg(ACCENT) } else { Style::default() };
    Paragraph::new(Line::from(vec![Span::styled(format!(" {}: ", label), label_style), Span::styled(value.to_string(), value_style)]))
}

fn checkbox_line(checked: bool, label: &str, focused: bool) -> Paragraph<'static> {
    let mark = if checked { "[x]" } else { "[ ]" };
    let style = if focused { Style::default().fg(ACCENT).add_modifier(Modifier::BOLD) } else { Style::default() };
    Paragraph::new(Line::from(vec![Span::styled(format!(" {} ", mark), style), Span::raw(label.to_string())]))
}

fn picker_line(value: &str, hint: &str, focused: bool) -> Paragraph<'static> {
    let style = if focused { Style::default().fg(ACCENT).add_modifier(Modifier::BOLD) } else { Style::default() };
    Paragraph::new(Line::from(vec![Span::styled(format!(" < {} > ", value), style), Span::styled(format!("  {}", hint), Style::default().fg(MUTED))]))
}

fn button_line(label: &str, focused: bool) -> Paragraph<'static> {
    let style = if focused {
        Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    };
    Paragraph::new(Span::styled(label.to_string(), style))
}

fn hint_bar(state: &WizardState, hint: &str) -> Paragraph<'static> {
    let line = match state.error.as_ref() {
        Some(err) => Line::from(Span::styled(err.clone(), Style::default().fg(ERROR).add_modifier(Modifier::BOLD))),
        None => Line::from(Span::styled(hint.to_string(), Style::default().fg(MUTED))),
    };
    Paragraph::new(line).block(Block::default().borders(Borders::ALL))
}

fn help_panel(text: &str) -> Paragraph<'static> {
    Paragraph::new(text.to_string())
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(MUTED))
}

fn verb_label(verb: Verb) -> &'static str {
    match verb {
        Verb::List => "List",
        Verb::Get => "Get",
        Verb::Create => "Create",
        Verb::Update => "Update",
        Verb::Delete => "Delete",
    }
}
