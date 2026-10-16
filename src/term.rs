//! Terminal setup / teardown guard.

use std::io::{self, Stdout};

use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

pub struct Term;

pub struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        Term::restore(&mut out);
    }
}

impl Term {
    pub fn init() -> io::Result<Stdout> {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let mut out = io::stdout();
            Term::restore(&mut out);
            default_hook(info);
        }));

        let mut out = io::stdout();
        enable_raw_mode()?;
        execute!(out, EnterAlternateScreen, Hide, EnableMouseCapture)?;
        Ok(out)
    }

    pub fn restore(out: &mut Stdout) {
        let _ = execute!(out, Show, DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
