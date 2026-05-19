use std::io;

use crossterm::{
    cursor::Show,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::error::{BlastError, BlastResult};

/// RAII guard for the TUI terminal lifecycle. `install()` enables raw mode +
/// alternate screen. `Drop` unconditionally restores: leave alt screen,
/// disable raw mode, show cursor — even on panic-unwind, so a panic mid-draw
/// doesn't wedge the user shell.
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn install() -> BlastResult<Self> {
        enable_raw_mode().map_err(BlastError::from)?;
        let mut stdout = io::stdout();
        match execute!(stdout, EnterAlternateScreen) {
            Ok(()) => Ok(Self),
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
        match execute!(stdout, LeaveAlternateScreen, Show) {
            Ok(()) => {}
            Err(_io) => {} // allow: best-effort cleanup on Drop; can't propagate
        }
        match disable_raw_mode() {
            Ok(()) => {}
            Err(_io) => {} // allow: best-effort cleanup on Drop; can't propagate
        }
    }
}
