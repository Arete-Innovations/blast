use std::io;

use crossterm::{
    cursor::Show,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::error::{BlastError, BlastResult};

/// RAII guard for the TUI terminal lifecycle. `install()` enables raw mode +
/// alternate screen (and optionally mouse capture). `Drop` unconditionally
/// restores: leave alt screen, disable raw mode, show cursor, disable mouse
/// — even on panic-unwind, so a panic mid-draw doesn't wedge the user shell.
pub struct TerminalGuard {
    mouse: bool,
}

impl TerminalGuard {
    pub fn install() -> BlastResult<Self> {
        Self::install_inner(false)
    }

    pub fn install_with_mouse() -> BlastResult<Self> {
        Self::install_inner(true)
    }

    fn install_inner(mouse: bool) -> BlastResult<Self> {
        enable_raw_mode().map_err(BlastError::from)?;
        let mut stdout = io::stdout();
        let setup_res = if mouse { execute!(stdout, EnterAlternateScreen, EnableMouseCapture) } else { execute!(stdout, EnterAlternateScreen) };
        match setup_res {
            Ok(()) => Ok(Self { mouse }),
            Err(e) => {
                match disable_raw_mode() {
                    Ok(()) => {}
                    Err(_io) => {} // allow: best-effort cleanup before propagating real error
                }
                Err(BlastError::from(e))
            }
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        if self.mouse {
            match execute!(stdout, LeaveAlternateScreen, DisableMouseCapture, Show) {
                Ok(()) => {}
                Err(_io) => {} // allow: best-effort cleanup on Drop; can't propagate
            }
        } else {
            match execute!(stdout, LeaveAlternateScreen, Show) {
                Ok(()) => {}
                Err(_io) => {} // allow: best-effort cleanup on Drop; can't propagate
            }
        }
        match disable_raw_mode() {
            Ok(()) => {}
            Err(_io) => {} // allow: best-effort cleanup on Drop; can't propagate
        }
    }
}
