use std::io;

use color_eyre::Result;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};

/// Leave the alternate screen, disable raw mode, run a closure, then restore
/// the TUI terminal state.
///
/// Use this to shell out to `$EDITOR`, `$PAGER`, or interactive `git` commands
/// from inside the TUI. The screen is cleared on resume so the TUI can redraw
/// from scratch.
///
/// # Example
///
/// ```rust,no_run
/// use roost::tui::suspend::suspend_and_run;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     suspend_and_run(|| {
///         std::process::Command::new("vi")
///             .arg("~/.bashrc")
///             .status()?;
///         Ok(())
///     })?;
///     Ok(())
/// }
/// ```
pub fn suspend_and_run<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    // Best-effort: leave alternate screen and disable raw mode.
    // These may fail when called outside a TUI (e.g. in unit tests),
    // so we ignore errors rather than aborting the user's closure.
    let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();

    let result = f();

    // Best-effort restore TUI state.
    let _ = enable_raw_mode();
    let _ = crossterm::execute!(io::stdout(), EnterAlternateScreen);
    let _ = crossterm::execute!(
        io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    );

    result
}

/// Convenience wrapper for running an external [`Command`] while suspended.
pub fn run_command(cmd: &mut std::process::Command) -> Result<std::process::ExitStatus> {
    suspend_and_run(|| {
        let status = cmd.status()?;
        Ok(status)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspend_and_run_no_op() {
        let result = suspend_and_run(|| Ok(42));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn suspend_and_run_propagates_error() {
        let result: Result<i32> = suspend_and_run(|| {
            Err(color_eyre::eyre::eyre!("intentional error"))
        });
        assert!(result.is_err());
    }
}
