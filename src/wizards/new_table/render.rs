use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::{
    emit,
    state::{ColumnsFocus, FormFocus, PreviewFocus, Screen, WizardState},
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const ERROR: Color = Color::LightRed;

pub fn draw(frame: &mut Frame<'_>, state: &mut WizardState) {
    let area = frame.area();
    match state.screen {
        Screen::Form => draw_form(frame, area, state),
        Screen::Columns => draw_columns(frame, area, state),
        Screen::Preview => draw_preview(frame, area, state),
    }
}

fn draw_form(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(15), Constraint::Length(3)])
        .split(area);

    frame.render_widget(title_block("blast — new table", "step 1/3 — table & policy"), chunks[0]);

    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // table name
            Constraint::Length(1),
            Constraint::Length(2), // header: auto-features
            Constraint::Length(1), // id_pk
            Constraint::Length(1), // created_at
            Constraint::Length(1), // updated_at
            Constraint::Length(1), // soft_delete
            Constraint::Length(1),
            Constraint::Length(2), // header: gen_level
            Constraint::Length(1), // gen_level row
            Constraint::Length(1),
            Constraint::Length(2), // header: verbs
            Constraint::Length(1), // List
            Constraint::Length(1), // Get
            Constraint::Length(1), // Create
            Constraint::Length(1), // Update
            Constraint::Length(1), // Delete
            Constraint::Length(1),
            Constraint::Length(1), // next button
            Constraint::Min(0),
        ])
        .split(chunks[1].inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 }));

    frame.render_widget(text_input_box(state.table_name.value(), state.form_focus == FormFocus::TableName, "Table name (snake_case)"), body_chunks[0]);

    frame.render_widget(section_label("Auto features"), body_chunks[2]);
    frame.render_widget(checkbox_line(state.id_pk, "id BIGSERIAL PRIMARY KEY", state.form_focus == FormFocus::IdPk), body_chunks[3]);
    frame.render_widget(checkbox_line(state.created_at, "created_at BIGINT (epoch)", state.form_focus == FormFocus::CreatedAt), body_chunks[4]);
    frame.render_widget(checkbox_line(state.updated_at, "updated_at BIGINT (epoch)", state.form_focus == FormFocus::UpdatedAt), body_chunks[5]);
    frame.render_widget(checkbox_line(state.soft_delete, "deleted_at BIGINT NULL  (soft-delete)", state.form_focus == FormFocus::SoftDelete), body_chunks[6]);

    frame.render_widget(section_label("Codegen depth"), body_chunks[8]);
    frame.render_widget(picker_line(&state.gen_level().label(), state.gen_level().description(), state.form_focus == FormFocus::GenLevel), body_chunks[9]);

    frame.render_widget(section_label("Verbs (each toggles independently — Space)"), body_chunks[11]);
    frame.render_widget(checkbox_line(state.verbs.list, "List   (GET /<table>)", state.form_focus == FormFocus::VerbList), body_chunks[12]);
    frame.render_widget(checkbox_line(state.verbs.get, "Get    (GET /<table>/:id)", state.form_focus == FormFocus::VerbGet), body_chunks[13]);
    frame.render_widget(checkbox_line(state.verbs.create, "Create (POST /<table>)", state.form_focus == FormFocus::VerbCreate), body_chunks[14]);
    frame.render_widget(checkbox_line(state.verbs.update, "Update (PATCH /<table>/:id)", state.form_focus == FormFocus::VerbUpdate), body_chunks[15]);
    frame.render_widget(checkbox_line(state.verbs.delete, "Delete (DELETE /<table>/:id)", state.form_focus == FormFocus::VerbDelete), body_chunks[16]);

    frame.render_widget(button_line("[ Next: Columns → ]", state.form_focus == FormFocus::Next), body_chunks[18]);

    frame.render_widget(hint_bar(state, "↑/↓/Tab focus  •  Space toggle  •  ←/→ cycle  •  Enter advance  •  Esc cancel"), chunks[2]);

    if state.form_focus == FormFocus::TableName {
        let inner = body_chunks[0];
        let cursor_x = inner.x + 1 + state.table_name.visual_cursor() as u16;
        let cursor_y = inner.y + 1;
        frame.set_cursor_position((cursor_x.min(inner.x + inner.width.saturating_sub(2)), cursor_y));
    }
}

fn draw_columns(frame: &mut Frame<'_>, area: Rect, state: &mut WizardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Percentage(45), Constraint::Min(12), Constraint::Length(3)])
        .split(area);

    let title = format!("blast — new table — columns  ({})", state.table_name.value());
    frame.render_widget(title_block(&title, "step 2/3 — define columns"), chunks[0]);

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
    frame.render_widget(list, chunks[1]);

    let draft_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // name
            Constraint::Length(1), // type
            Constraint::Length(1), // not null
            Constraint::Length(1), // public-visible
            Constraint::Length(1), // validator
            Constraint::Length(1),
            Constraint::Length(1), // add row
            Constraint::Length(1), // delete last row
            Constraint::Length(1),
            Constraint::Length(1), // back / done row (split horizontal)
            Constraint::Min(0),
        ])
        .split(chunks[2].inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 }));

    frame.render_widget(text_input_inline("name", state.draft.name.value(), state.columns_focus == ColumnsFocus::DraftName), draft_chunks[0]);

    let type_label = match state.current_draft_type() {
        Some(ty) => ty.label(),
        None => "(no types available)".to_string(),
    };
    frame.render_widget(picker_line(&type_label, "Type — ←/→ cycles", state.columns_focus == ColumnsFocus::DraftType), draft_chunks[1]);
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
        picker_line(state.draft.current_validator().label(), "Validator — ←/→ cycles", state.columns_focus == ColumnsFocus::DraftValidator),
        draft_chunks[4],
    );

    frame.render_widget(button_line("[ + Add column ]", state.columns_focus == ColumnsFocus::AddColumn), draft_chunks[6]);
    frame.render_widget(button_line("[ – Delete last column ]", state.columns_focus == ColumnsFocus::DeleteLast), draft_chunks[7]);

    let nav_buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(draft_chunks[9]);
    frame.render_widget(button_line("[ ← Back ]", state.columns_focus == ColumnsFocus::Back), nav_buttons[0]);
    frame.render_widget(button_line("[ Done — Preview → ]", state.columns_focus == ColumnsFocus::Done), nav_buttons[1]);

    frame.render_widget(hint_bar(state, "↑/↓/Tab focus  •  Space toggle  •  ←/→ cycle types  •  Enter add/back/done  •  Esc cancel"), chunks[3]);

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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(20), Constraint::Length(3), Constraint::Length(3)])
        .split(area);

    let title = format!("blast — new table — preview  ({})", state.table_name.value());
    frame.render_widget(title_block(&title, "step 3/3 — review & commit"), chunks[0]);

    let arts = emit::build(state);
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(20), Constraint::Percentage(40)])
        .split(chunks[1]);

    frame.render_widget(Paragraph::new(arts.up_sql).block(Block::default().borders(Borders::ALL).title("up.sql")).wrap(Wrap { trim: false }), body[0]);
    frame.render_widget(Paragraph::new(arts.down_sql).block(Block::default().borders(Borders::ALL).title("down.sql")).wrap(Wrap { trim: false }), body[1]);
    frame.render_widget(
        Paragraph::new(arts.resource_ron)
            .block(Block::default().borders(Borders::ALL).title(format!("storage/blast/state/resources/{}.ron", state.table_name.value())))
            .wrap(Wrap { trim: false }),
        body[2],
    );

    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2].inner(ratatui::layout::Margin { horizontal: 2, vertical: 0 }));

    frame.render_widget(button_line("[ ← Back ]", state.preview_focus == PreviewFocus::Back), buttons[0]);
    frame.render_widget(button_line("[ Commit + Run Pipeline → ]", state.preview_focus == PreviewFocus::Commit), buttons[1]);

    frame.render_widget(hint_bar(state, "Tab/←/→ switch button  •  Enter activate  •  Esc cancel"), chunks[3]);
}

// ── widget helpers ──────────────────────────────────────────────────────────

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
