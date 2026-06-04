use crossterm::event::KeyEvent;

use crate::tui::main_view::state::MainViewState;

/// Side effects produced by a single keypress.
///
/// The event loop collects actions, then processes them after the key handler
/// returns so that state mutations and I/O are separated from input parsing.
#[derive(Debug)]
pub enum Action {
    Quit,
    SetStatus(String),
    AutoCommit(String),
    RemoveApp(String),
    OpenEditor(std::path::PathBuf),
    OpenPager(String),
    Sync,
    SwitchProfile(String),
    CreateProfile { name: String, copy_current: bool },
    DeleteProfile(String),
    SetPrimary { app: String, path: std::path::PathBuf },
    ImportApp { app: String, source_profile: String },
    CopyApp { app: String, target_profile: String },
    AddIgnore(String),
    RemoveIgnore(String),
    Undo,
    Rollback(String),
    Refresh,
    Nop,
}

/// Process a key event and return zero or more actions.
///
/// Routing order (first match wins):
/// 1. Confirm dialog
/// 2. Search overlay
/// 3. Help dialog
/// 4. Profile dialog
/// 5. Ignore dialog
/// 6. Git log dialog
/// 7. Undo dialog
/// 8. App link dialog
/// 9. Diff view
/// 10. Base panel input (Apps or Files)
pub fn handle_event(_state: &mut MainViewState, _key: KeyEvent) -> Vec<Action> {
    // TODO: implement in Stream 2 (event.rs)
    vec![Action::Nop]
}
