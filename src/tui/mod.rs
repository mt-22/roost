pub mod confirm;
pub mod main_view;
pub mod search;
pub mod suspend;

use std::io;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};

/// Global flag set by Ctrl-C handler. Polled by TUI event loops to exit.
pub static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

/// Initialize global TUI handlers (panic hook, Ctrl-C) exactly once.
///
/// Safe to call any number of times — the second and subsequent calls are no-ops.
pub fn init() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            original_hook(info);
        }));

        let _ = ctrlc::set_handler(|| {
            SHOULD_EXIT.store(true, Ordering::SeqCst);
        });
    });
}
