use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

use crate::error::{BlastError, BlastResult};

pub fn pick<S: AsRef<str>>(prompt: &str, items: &[S]) -> BlastResult<Option<usize>> {
    if items.is_empty() {
        return Err(BlastError::Invalid("list_select: items must not be empty".to_string()));
    }

    enable_raw_mode().map_err(io_to_blast)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(io_to_blast)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(io_to_blast)?;

    let mut state = ListState::default();
    state.select(Some(0));

    let mut chosen: Option<usize> = None;
    let labels: Vec<&str> = items.iter().map(|s| s.as_ref()).collect();

    let outcome: BlastResult<()> = loop {
        let draw_res = terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
                .split(f.area());

            let header = Paragraph::new(prompt).block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let list_items: Vec<ListItem> = labels.iter().map(|l| ListItem::new(*l)).collect();
            let list = List::new(list_items)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("> ");
            f.render_stateful_widget(list, chunks[1], &mut state);

            let footer = Paragraph::new("↑/↓ select  Enter confirm  Esc/Ctrl-C cancel");
            f.render_widget(footer, chunks[2]);
        });
        if let Err(e) = draw_res {
            break Err(io_to_blast(e));
        }

        let poll_res = event::poll(Duration::from_millis(200));
        let evt_ready = match poll_res {
            Ok(v) => v,
            Err(e) => break Err(io_to_blast(e)),
        };
        if !evt_ready {
            continue;
        }
        let evt = match event::read() {
            Ok(v) => v,
            Err(e) => break Err(io_to_blast(e)),
        };

        if let Event::Key(k) = evt {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match (k.code, k.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => {
                    chosen = None;
                    break Ok(());
                }
                (KeyCode::Enter, _) => {
                    chosen = state.selected();
                    break Ok(());
                }
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                    let cur = state.selected().unwrap_or(0); // allow: empty list rejected at fn entry
                    let next = if cur == 0 { labels.len() - 1 } else { cur - 1 };
                    state.select(Some(next));
                }
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                    let cur = state.selected().unwrap_or(0); // allow: empty list rejected at fn entry
                    let next = (cur + 1) % labels.len();
                    state.select(Some(next));
                }
                (KeyCode::Home, _) | (KeyCode::Char('g'), _) => state.select(Some(0)),
                (KeyCode::End, _) | (KeyCode::Char('G'), _) => state.select(Some(labels.len() - 1)),
                _passthrough => {}
            }
        }
    };

    disable_raw_mode().map_err(io_to_blast)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(io_to_blast)?;
    terminal.show_cursor().map_err(io_to_blast)?;

    match outcome {
        Ok(()) => Ok(chosen),
        Err(e) => Err(e),
    }
}

fn io_to_blast(e: io::Error) -> BlastError {
    BlastError::from(e)
}
