use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use tui_input::{backend::crossterm::EventHandler, Input};

use crate::{
    error::{BlastError, BlastResult},
    tui_widgets::terminal_guard::TerminalGuard,
};

pub fn ask(prompt: &str, default: Option<&str>) -> BlastResult<Option<String>> {
    let _guard = TerminalGuard::install()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(io_to_blast)?;

    let mut input: Input = Input::default();
    match default {
        Some(d) => input = input.with_value(d.to_string()),
        None => {}
    }

    let mut value: Option<String> = None;

    let outcome: BlastResult<()> = loop {
        let draw_res = terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(1)])
                .split(f.area());

            let header = Paragraph::new(prompt).block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let inp = Paragraph::new(input.value()).block(Block::default().borders(Borders::ALL));
            f.render_widget(inp, chunks[1]);

            let footer = Paragraph::new("Enter confirm  Esc/Ctrl-C cancel");
            f.render_widget(footer, chunks[2]);

            f.set_cursor_position((chunks[1].x + 1 + input.visual_cursor() as u16, chunks[1].y + 1));
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

        if let Event::Key(k) = &evt {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match (k.code, k.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => {
                    value = None;
                    break Ok(());
                }
                (KeyCode::Enter, _) => {
                    value = Some(input.value().to_string());
                    break Ok(());
                }
                _passthrough => {
                    input.handle_event(&evt);
                }
            }
        }
    };

    match outcome {
        Ok(()) => Ok(value),
        Err(e) => Err(e),
    }
}

fn io_to_blast(e: io::Error) -> BlastError {
    BlastError::from(e)
}
