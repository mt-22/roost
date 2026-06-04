use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use std::path::PathBuf;

use crate::tui::confirm::{ConfirmAction, ConfirmDialog};
use crate::tui::main_view::state::{Focus, MainViewState, SearchState, SearchTarget};

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
    OpenEditor(PathBuf),
    OpenPager(String),
    Sync,
    SwitchProfile(String),
    CreateProfile { name: String, copy_current: bool },
    DeleteProfile(String),
    SetPrimary { app: String, path: PathBuf },
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
/// 3. Base panel input (Apps or Files)
pub fn handle_event(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    if key.kind != KeyEventKind::Press {
        return vec![Action::Nop];
    }

    // 1. Confirm dialog has highest priority.
    if state.confirm_dialog.is_some() {
        return handle_confirm(state, key);
    }

    // 2. Search overlay.
    if state.search.is_some() {
        return handle_search(state, key);
    }

    // 3. Base panel input.
    handle_base(state, key)
}

// ------------------------------------------------------------------
// Confirm dialog
// ------------------------------------------------------------------

fn handle_confirm(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    let Some(ref mut dialog) = state.confirm_dialog else {
        return vec![Action::Nop];
    };

    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            dialog.confirm();
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            dialog.cancel();
        }
        _ => {}
    }

    // If the dialog was just resolved, process the action now.
    if let Some(result) = dialog.take_result() {
        let action = dialog.action;
        state.confirm_dialog = None;

        if !result {
            return vec![Action::Nop];
        }

        return match action {
            ConfirmAction::Confirm => vec![Action::Quit],
            ConfirmAction::Discard => {
                // Discard was used for "quit with unsaved changes".
                // In a future refactor we may distinguish actions per dialog.
                vec![Action::Quit]
            }
        };
    }

    vec![Action::Nop]
}

// ------------------------------------------------------------------
// Search overlay
// ------------------------------------------------------------------

fn handle_search(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Esc => {
            state.search = None;
            return vec![Action::Nop];
        }
        KeyCode::Enter => {
            let actions = apply_search_selection(state);
            state.search = None;
            return actions;
        }
        KeyCode::Backspace => {
            if let Some(ref mut search) = state.search {
                if !search.query.is_empty() {
                    search.query.pop();
                    apply_search_filter(state);
                }
            }
            return vec![Action::Nop];
        }
        KeyCode::Up => {
            if let Some(ref mut search) = state.search {
                search.engine.move_up();
            }
            sync_search_cursor(state);
            return vec![Action::Nop];
        }
        KeyCode::Down => {
            if let Some(ref mut search) = state.search {
                search.engine.move_down();
            }
            sync_search_cursor(state);
            return vec![Action::Nop];
        }
        KeyCode::Char('k') => {
            if let Some(ref mut search) = state.search {
                search.engine.move_up();
            }
            sync_search_cursor(state);
            return vec![Action::Nop];
        }
        KeyCode::Char('j') => {
            if let Some(ref mut search) = state.search {
                search.engine.move_down();
            }
            sync_search_cursor(state);
            return vec![Action::Nop];
        }
        KeyCode::Char(c) => {
            if let Some(ref mut search) = state.search {
                search.query.push(c);
            }
            apply_search_filter(state);
            return vec![Action::Nop];
        }
        _ => vec![Action::Nop],
    }
}

fn apply_search_filter(state: &mut MainViewState) {
    let target = state.search.as_ref().expect("search active").target;

    match target {
        SearchTarget::Apps => {
            let names: Vec<String> = state.apps_in_active_profile()
                .into_iter()
                .cloned()
                .collect();
            let idx = {
                let search = state.search.as_mut().expect("search active");
                search.engine.filter(&names);
                search.engine.selected_index()
            };
            if let Some(idx) = idx {
                state.app_cursor = idx;
                state.sync_miller_to_selected_app();
            }
        }
        SearchTarget::Files => {
            let names: Vec<String> = state
                .miller
                .current_entries()
                .iter()
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect();
            let idx = {
                let search = state.search.as_mut().expect("search active");
                search.engine.filter(&names);
                search.engine.selected_index()
            };
            if let Some(idx) = idx {
                state.miller.set_current_cursor(idx);
            }
        }
    }
}

fn sync_search_cursor(state: &mut MainViewState) {
    let (target, idx) = {
        let search = state.search.as_ref().expect("search active");
        (search.target, search.engine.selected_index())
    };
    if let Some(idx) = idx {
        match target {
            SearchTarget::Apps => {
                state.app_cursor = idx;
                state.sync_miller_to_selected_app();
            }
            SearchTarget::Files => {
                state.miller.set_current_cursor(idx);
            }
        }
    }
}

fn apply_search_selection(state: &mut MainViewState) -> Vec<Action> {
    let search = state.search.as_ref().expect("search active");
    if let Some(idx) = search.engine.selected_index() {
        match search.target {
            SearchTarget::Apps => {
                state.app_cursor = idx;
                state.sync_miller_to_selected_app();
            }
            SearchTarget::Files => {
                state.miller.set_current_cursor(idx);
            }
        }
    }
    vec![Action::Nop]
}

// ------------------------------------------------------------------
// Base panel input
// ------------------------------------------------------------------

fn handle_base(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    // Any keypress at the base layer clears stale status messages.
    state.status_message = None;

    match key.code {
        // Navigation
        KeyCode::Char('j') => {
            match state.focus {
                Focus::AppsPanel => state.app_cursor_down(),
                Focus::FilesPanel => state.miller.move_down(),
            }
            vec![Action::Nop]
        }
        KeyCode::Char('k') => {
            match state.focus {
                Focus::AppsPanel => state.app_cursor_up(),
                Focus::FilesPanel => state.miller.move_up(),
            }
            vec![Action::Nop]
        }
        KeyCode::Tab => {
            state.focus = state.focus.toggle();
            vec![Action::Nop]
        }
        // h/l as "enter/exit" the focused area
        KeyCode::Char('h') => {
            match state.focus {
                Focus::AppsPanel => vec![Action::Nop], // no-op from app list
                Focus::FilesPanel => {
                    if state.miller.is_at_root() {
                        state.focus = Focus::AppsPanel;
                    } else {
                        state.miller.navigate_up();
                    }
                    vec![Action::Nop]
                }
            }
        }
        KeyCode::Char('l') => {
            match state.focus {
                Focus::AppsPanel => {
                    state.focus = Focus::FilesPanel;
                    vec![Action::Nop]
                }
                Focus::FilesPanel => {
                    if state.miller.current_cursor_is_dir() {
                        state.miller.navigate_down();
                        vec![Action::Nop]
                    } else if let Some(path) = state.miller.current_cursor_path() {
                        vec![Action::OpenEditor(path)]
                    } else {
                        vec![Action::Nop]
                    }
                }
            }
        }

        // Search
        KeyCode::Char('/') => {
            let target = match state.focus {
                Focus::AppsPanel => SearchTarget::Apps,
                Focus::FilesPanel => SearchTarget::Files,
            };
            let mut engine = crate::tui::search::FuzzyEngine::new();
            let names: Vec<String> = match target {
                SearchTarget::Apps => state
                    .apps_in_active_profile()
                    .into_iter()
                    .cloned()
                    .collect(),
                SearchTarget::Files => state
                    .miller
                    .current_entries()
                    .iter()
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                    .collect(),
            };
            engine.filter(&names);
            state.search = Some(SearchState {
                engine,
                query: String::new(),
                target,
            });
            vec![Action::Nop]
        }

        // Quit
        KeyCode::Char('q') | KeyCode::Esc => {
            if state.pending_auto_commit.is_some() {
                state.confirm_dialog = Some(ConfirmDialog::destructive(
                    "Quit",
                    "You have unsaved changes. Quit anyway?",
                ));
                vec![Action::Nop]
            } else {
                vec![Action::Quit]
            }
        }

        // Apps-panel actions
        KeyCode::Char('o') if state.focus == Focus::AppsPanel => {
            if let Some(app) = state.selected_app().cloned() {
                if let Some(path) = state
                    .shared
                    .apps
                    .get(&app)
                    .and_then(|a| a.primary_config.clone())
                {
                    // Resolve tilde-relative path via local link_paths if available.
                    let resolved = state
                        .local
                        .link_paths
                        .get(&app)
                        .cloned()
                        .unwrap_or(path);
                    return vec![Action::OpenEditor(resolved)];
                }
            }
            vec![Action::SetStatus("No primary config for this app".to_string())]
        }
        KeyCode::Char('x') if state.focus == Focus::AppsPanel => {
            if let Some(app) = state.selected_app().cloned() {
                state.confirm_dialog = Some(ConfirmDialog::destructive(
                    "Remove App",
                    &format!("Remove '{}' from roost?", app),
                ));
            }
            vec![Action::Nop]
        }
        KeyCode::Char('f') if state.focus == Focus::AppsPanel => {
            vec![Action::SetStatus(
                "Import-from dialog not yet implemented — use `roost import`".to_string(),
            )]
        }
        KeyCode::Char('m') if state.focus == Focus::AppsPanel => {
            vec![Action::SetStatus(
                "Paste-into dialog not yet implemented — use `roost copy`".to_string(),
            )]
        }

        // Files-panel actions
        KeyCode::Char('e') | KeyCode::Enter if state.focus == Focus::FilesPanel => {
            if let Some(path) = state.miller.current_cursor_path() {
                vec![Action::OpenEditor(path)]
            } else {
                vec![Action::Nop]
            }
        }
        KeyCode::Char('p') if state.focus == Focus::FilesPanel => {
            if let Some(app) = state.selected_app().cloned() {
                if let Some(path) = state.miller.current_cursor_path() {
                    state.confirm_dialog = Some(ConfirmDialog::affirmative(
                        "Set Primary",
                        &format!("Set '{}' as primary config for '{}'?", path.display(), app),
                    ));
                }
            }
            vec![Action::Nop]
        }

        // Global actions
        KeyCode::Char('s') => vec![Action::Sync],
        KeyCode::Char('a') => vec![Action::SetStatus(
            "Add-app dialog not yet implemented — use `roost add <path>`".to_string(),
        )],
        KeyCode::Char('i') => vec![Action::SetStatus(
            "Ignore dialog not yet implemented — use `roost ignore`".to_string(),
        )],
        KeyCode::Char('P') => vec![Action::SetStatus(
            "Profile dialog not yet implemented — use `roost profile`".to_string(),
        )],
        KeyCode::Char('g') => vec![Action::SetStatus(
            "Git log dialog not yet implemented — use `roost log`".to_string(),
        )],
        KeyCode::Char('d') => vec![Action::OpenPager("git_diff".to_string())],
        KeyCode::Char('u') => vec![Action::SetStatus(
            "Undo dialog not yet implemented — use `roost undo`".to_string(),
        )],
        KeyCode::Char('?') => vec![Action::SetStatus(
            "Help dialog not yet implemented".to_string(),
        )],

        _ => vec![Action::Nop],
    }
}
