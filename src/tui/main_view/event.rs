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

    // 3. Help dialog.
    if state.help_dialog.is_some() {
        return handle_help(state, key);
    }

    // 4. Profile dialog.
    if state.profile_dialog.is_some() {
        return handle_profile(state, key);
    }

    // 5. Ignore dialog.
    if state.ignore_dialog.is_some() {
        return handle_ignore(state, key);
    }

    // 6. Git log dialog.
    if state.git_log_dialog.is_some() {
        return handle_git_log(state, key);
    }

    // 7. Undo dialog.
    if state.undo_dialog.is_some() {
        return handle_undo(state, key);
    }

    // 8. App link dialog.
    if state.app_link_dialog.is_some() {
        return handle_app_link(state, key);
    }

    // 9. Diff view dialog.
    if state.diff_view.is_some() {
        return handle_diff_view(state, key);
    }

    // 10. Base panel input.
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
            // Clear any pending rollback marker on cancel.
            if let Some(ref msg) = state.status_message {
                if msg.starts_with("rollback_pending:") {
                    state.status_message = None;
                }
            }
            return vec![Action::Nop];
        }

        // Check for a pending rollback marker set by the git log dialog.
        if let Some(ref msg) = state.status_message {
            if let Some(hash) = msg.strip_prefix("rollback_pending:") {
                let hash = hash.to_string();
                state.status_message = None;
                return vec![Action::Rollback(hash)];
            }
        }

        return match action {
            ConfirmAction::Confirm => vec![Action::Quit],
            ConfirmAction::Discard => vec![Action::Quit],
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
            state.app_link_dialog = Some(crate::tui::main_view::dialogs::AppLinkState::new(
                crate::tui::main_view::dialogs::AppLinkAction::Import,
            ));
            vec![Action::Nop]
        }
        KeyCode::Char('m') if state.focus == Focus::AppsPanel => {
            state.app_link_dialog = Some(crate::tui::main_view::dialogs::AppLinkState::new(
                crate::tui::main_view::dialogs::AppLinkAction::Copy,
            ));
            vec![Action::Nop]
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
        KeyCode::Char('i') => {
            state.ignore_dialog = Some(crate::tui::main_view::dialogs::IgnoreState::new());
            vec![Action::Nop]
        }
        KeyCode::Char('P') => {
            state.profile_dialog = Some(crate::tui::main_view::dialogs::ProfileState::new());
            vec![Action::Nop]
        }
        KeyCode::Char('g') => {
            let roost_dir = state.roost_dir.clone();
            match crate::git::log(&roost_dir, 50) {
                Ok(commits) => {
                    state.git_log_dialog = Some(crate::tui::main_view::dialogs::GitLogState::new(commits));
                }
                Err(e) => {
                    return vec![Action::SetStatus(format!("Git log error: {}", e))];
                }
            }
            vec![Action::Nop]
        }
        KeyCode::Char('d') => {
            let roost_dir = state.roost_dir.clone();
            match crate::git::diff(&roost_dir) {
                Ok(diff_text) => {
                    state.diff_view = Some(crate::tui::main_view::dialogs::DiffViewState::new(&diff_text));
                }
                Err(e) => {
                    return vec![Action::SetStatus(format!("Diff error: {}", e))];
                }
            }
            vec![Action::Nop]
        }
        KeyCode::Char('u') => {
            let roost_dir = state.roost_dir.clone();
            match crate::git::log(&roost_dir, 1) {
                Ok(commits) if !commits.is_empty() => {
                    let msg = format!("Undo last commit?\n{}  {}", &commits[0].hash[..7], commits[0].message);
                    state.undo_dialog = Some(crate::tui::main_view::dialogs::UndoState::new(msg));
                }
                _ => {
                    return vec![Action::SetStatus("No commits to undo".to_string())];
                }
            }
            vec![Action::Nop]
        }
        KeyCode::Char('?') => {
            state.help_dialog = Some(crate::tui::main_view::dialogs::HelpState::new());
            vec![Action::Nop]
        }

        _ => vec![Action::Nop],
    }
}

// ------------------------------------------------------------------
// Help dialog
// ------------------------------------------------------------------

fn handle_help(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    use crate::tui::main_view::dialogs::KEYBINDS;

    let Some(ref mut help) = state.help_dialog else {
        return vec![Action::Nop];
    };

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.help_dialog = None;
            vec![Action::Nop]
        }
        KeyCode::Up | KeyCode::Char('k') => {
            help.move_up();
            vec![Action::Nop]
        }
        KeyCode::Down | KeyCode::Char('j') => {
            help.move_down(KEYBINDS.len());
            vec![Action::Nop]
        }
        _ => vec![Action::Nop],
    }
}

// ------------------------------------------------------------------
// Profile dialog
// ------------------------------------------------------------------

fn handle_profile(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    use crate::tui::main_view::dialogs::ProfileMode;

    let Some(ref mut profile) = state.profile_dialog else {
        return vec![Action::Nop];
    };

    let mode = profile.mode;

    match key.code {
        KeyCode::Esc => {
            state.profile_dialog = None;
            vec![Action::Nop]
        }
        KeyCode::Tab => {
            profile.cycle_mode();
            vec![Action::Nop]
        }
        KeyCode::Up | KeyCode::Char('k') => {
            profile.move_up();
            vec![Action::Nop]
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = match mode {
                ProfileMode::Switch | ProfileMode::Delete => state.shared.profiles.len(),
                ProfileMode::Create => 0,
            };
            profile.move_down(max);
            vec![Action::Nop]
        }
        KeyCode::Enter => {
            match mode {
                ProfileMode::Switch => {
                    let names: Vec<&String> = {
                        let mut v: Vec<&String> = state.shared.profiles.keys().collect();
                        v.sort();
                        v
                    };
                    if let Some(name) = names.get(profile.cursor) {
                        let name = name.to_string();
                        state.profile_dialog = None;
                        return vec![Action::SwitchProfile(name)];
                    }
                }
                ProfileMode::Create => {
                    let (name, copy_current) = {
                        let name = profile.input.trim().to_string();
                        let copy = profile.copy_current;
                        (name, copy)
                    };
                    if !name.is_empty() {
                        state.profile_dialog = None;
                        return vec![Action::CreateProfile {
                            name,
                            copy_current,
                        }];
                    }
                }
                ProfileMode::Delete => {
                    let names: Vec<&String> = {
                        let mut v: Vec<&String> = state.shared.profiles.keys().collect();
                        v.sort();
                        v
                    };
                    if let Some(name) = names.get(profile.cursor) {
                        let name = name.to_string();
                        if name == state.local.active_profile {
                            return vec![Action::SetStatus(
                                "Cannot delete the active profile".to_string(),
                            )];
                        }
                        state.profile_dialog = None;
                        return vec![Action::DeleteProfile(name)];
                    }
                }
            }
            vec![Action::Nop]
        }
        KeyCode::Char(' ') if mode == ProfileMode::Create => {
            profile.copy_current = !profile.copy_current;
            vec![Action::Nop]
        }
        KeyCode::Char(c) if mode == ProfileMode::Create => {
            profile.input.push(c);
            vec![Action::Nop]
        }
        KeyCode::Backspace if mode == ProfileMode::Create => {
            profile.input.pop();
            vec![Action::Nop]
        }
        _ => vec![Action::Nop],
    }
}

// ------------------------------------------------------------------
// Ignore dialog
// ------------------------------------------------------------------

fn handle_ignore(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    use crate::tui::main_view::dialogs::IgnoreMode;

    let Some(ref mut ignore) = state.ignore_dialog else {
        return vec![Action::Nop];
    };

    let mode = ignore.mode;

    match key.code {
        KeyCode::Esc => {
            state.ignore_dialog = None;
            vec![Action::Nop]
        }
        KeyCode::Tab => {
            ignore.cycle_mode();
            vec![Action::Nop]
        }
        KeyCode::Up | KeyCode::Char('k') => {
            ignore.move_up();
            vec![Action::Nop]
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = match mode {
                IgnoreMode::Remove => state.shared.ignored.len(),
                IgnoreMode::Add => 0,
            };
            ignore.move_down(max);
            vec![Action::Nop]
        }
        KeyCode::Enter => {
            match mode {
                IgnoreMode::Add => {
                    let pattern = ignore.input.trim().to_string();
                    if !pattern.is_empty() {
                        state.ignore_dialog = None;
                        return vec![Action::AddIgnore(pattern)];
                    }
                }
                IgnoreMode::Remove => {
                    let patterns: Vec<&String> = {
                        let mut v: Vec<&String> = state.shared.ignored.iter().collect();
                        v.sort();
                        v
                    };
                    if let Some(pat) = patterns.get(ignore.cursor) {
                        let pat = pat.to_string();
                        state.ignore_dialog = None;
                        return vec![Action::RemoveIgnore(pat)];
                    }
                }
            }
            vec![Action::Nop]
        }
        KeyCode::Char(c) if mode == IgnoreMode::Add => {
            ignore.input.push(c);
            vec![Action::Nop]
        }
        KeyCode::Backspace if mode == IgnoreMode::Add => {
            ignore.input.pop();
            vec![Action::Nop]
        }
        _ => vec![Action::Nop],
    }
}

// ------------------------------------------------------------------
// Git log dialog
// ------------------------------------------------------------------

fn handle_git_log(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    let Some(ref mut git_log) = state.git_log_dialog else {
        return vec![Action::Nop];
    };

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.git_log_dialog = None;
            vec![Action::Nop]
        }
        KeyCode::Up | KeyCode::Char('k') => {
            git_log.move_up();
            vec![Action::Nop]
        }
        KeyCode::Down | KeyCode::Char('j') => {
            git_log.move_down();
            vec![Action::Nop]
        }
        KeyCode::Char('r') => {
            if let Some(hash) = git_log.selected_hash().map(|s| s.to_string()) {
                state.git_log_dialog = None;
                state.confirm_dialog = Some(ConfirmDialog::destructive(
                    "Rollback",
                    &format!(
                        "Rollback to {}?\n\nWARNING: This is a destructive hard reset and cannot be easily undone.",
                        &hash[..7]
                    ),
                ));
                vec![Action::SetStatus(format!("rollback_pending:{}", hash))]
            } else {
                vec![Action::Nop]
            }
        }
        _ => vec![Action::Nop],
    }
}

// ------------------------------------------------------------------
// Undo dialog
// ------------------------------------------------------------------

fn handle_undo(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    let Some(ref _undo) = state.undo_dialog else {
        return vec![Action::Nop];
    };

    match key.code {
        KeyCode::Esc => {
            state.undo_dialog = None;
            vec![Action::Nop]
        }
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            state.undo_dialog = None;
            vec![Action::Undo]
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            state.undo_dialog = None;
            vec![Action::Nop]
        }
        _ => vec![Action::Nop],
    }
}

// ------------------------------------------------------------------
// App link dialog
// ------------------------------------------------------------------

fn handle_app_link(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    use crate::tui::main_view::dialogs::AppLinkStep;

    let Some(ref mut app_link) = state.app_link_dialog else {
        return vec![Action::Nop];
    };

    let step = app_link.step;

    match key.code {
        KeyCode::Esc => {
            state.app_link_dialog = None;
            vec![Action::Nop]
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app_link.move_up();
            vec![Action::Nop]
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = match step {
                AppLinkStep::PickProfile => state.shared.profiles.len(),
                AppLinkStep::PickApp => {
                    if let Some(ref profile) = app_link.selected_profile {
                        state.shared.profiles.get(profile).map(|p| p.apps.len()).unwrap_or(0)
                    } else {
                        0
                    }
                }
                AppLinkStep::ConfirmCopy => 0,
            };
            app_link.move_down(max);
            vec![Action::Nop]
        }
        KeyCode::Enter => {
            match step {
                AppLinkStep::PickProfile => {
                    let mut names: Vec<&String> = state.shared.profiles.keys().collect();
                    names.sort();
                    if let Some(name) = names.get(app_link.cursor) {
                        let name = name.to_string();
                        app_link.selected_profile = Some(name);
                        app_link.advance_step();
                    }
                }
                AppLinkStep::PickApp => {
                    if let Some(ref profile) = app_link.selected_profile {
                        let mut apps: Vec<&String> = state
                            .shared
                            .profiles
                            .get(profile)
                            .map(|p| p.apps.iter().collect())
                            .unwrap_or_default();
                        apps.sort();
                        if let Some(app) = apps.get(app_link.cursor) {
                            let app_name = app.to_string();
                            let source = profile.clone();
                            state.app_link_dialog = None;
                            return vec![Action::ImportApp {
                                app: app_name,
                                source_profile: source,
                            }];
                        }
                    }
                }
                AppLinkStep::ConfirmCopy => {
                    if let Some(ref profile) = app_link.selected_profile {
                        let target = profile.clone();
                        if let Some(app) = state.selected_app().cloned() {
                            state.app_link_dialog = None;
                            return vec![Action::CopyApp {
                                app,
                                target_profile: target,
                            }];
                        }
                    }
                }
            }
            vec![Action::Nop]
        }
        _ => vec![Action::Nop],
    }
}

// ------------------------------------------------------------------
// Diff view dialog
// ------------------------------------------------------------------

fn handle_diff_view(state: &mut MainViewState, key: KeyEvent) -> Vec<Action> {
    let Some(ref mut diff) = state.diff_view else {
        return vec![Action::Nop];
    };

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.diff_view = None;
            vec![Action::Nop]
        }
        KeyCode::Up | KeyCode::Char('k') => {
            diff.scroll_up();
            vec![Action::Nop]
        }
        KeyCode::Down | KeyCode::Char('j') => {
            diff.scroll_down(1);
            vec![Action::Nop]
        }
        KeyCode::Char('e') => {
            state.diff_view = None;
            vec![Action::OpenPager("git_diff".to_string())]
        }
        _ => vec![Action::Nop],
    }
}
