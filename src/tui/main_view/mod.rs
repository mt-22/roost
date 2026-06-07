pub mod dialogs;
pub mod event;
pub mod state;
pub mod ui;

pub use state::MainViewState;

use std::collections::HashSet;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use color_eyre::Result;
use crossterm::event::{Event, KeyEventKind, poll as crossterm_poll, read as crossterm_read};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::app::{self, LocalAppConfig, SharedAppConfig};
use crate::git;
use crate::linker;
use crate::pager;
use crate::tui::main_view::event::{Action, handle_event};
use crate::tui::main_view::ui::render;
use crate::tui::suspend::suspend_and_run;

/// Minimum terminal dimensions for the main TUI.
/// Below these, a "terminal too small" placeholder is shown instead.
const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 12;

/// Launch the main daily-use TUI.
///
/// Sets up the terminal, runs the event loop, processes actions, and restores
/// the terminal on exit.
pub fn run(roost_dir: PathBuf, shared: SharedAppConfig, local: LocalAppConfig) -> Result<()> {
    crate::tui::init();
    crate::tui::SHOULD_EXIT.store(false, Ordering::SeqCst);
    let mut terminal = setup_terminal()?;

    run_loop(&mut terminal, roost_dir, shared, local);

    restore_terminal(&mut terminal)?;

    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    crossterm::execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

/// Render a placeholder when the terminal is too small to display the main UI.
fn render_too_small(frame: &mut Frame, size: Rect) {
    let message = format!(
        "Terminal too small ({}x{}). Minimum: {}x{}.",
        size.width, size.height, MIN_WIDTH, MIN_HEIGHT
    );
    let line = Line::from(vec![Span::styled(
        message,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )]);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(Clear, size);
    frame.render_widget(paragraph, size);
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    roost_dir: PathBuf,
    shared: SharedAppConfig,
    local: LocalAppConfig,
) {
    let mut state = MainViewState::new(roost_dir, shared, local);

    loop {
        if state.needs_redraw {
            // Best-effort clear — terminal may not be fully initialized yet.
            let _ = terminal.clear();
            state.needs_redraw = false;
        }

        let size = match terminal.size() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if size.width < MIN_WIDTH || size.height < MIN_HEIGHT {
            let area = Rect::new(0, 0, size.width, size.height);
            // Terminal draw errors are non-fatal during the render loop.
            let _ = terminal.draw(|f| {
                render_too_small(f, area);
            });
        } else {
            // Terminal draw errors are non-fatal during the render loop.
            let _ = terminal.draw(|f| {
                render(&mut state, f);
            });
        }

        if crate::tui::SHOULD_EXIT.load(Ordering::Relaxed) {
            state.quit = true;
        }

        if state.quit {
            break;
        }

        if match crossterm_poll(std::time::Duration::from_millis(100)) {
            Ok(b) => b,
            Err(_) => continue,
        } {
            let key = match crossterm_read() {
                Ok(e) => e,
                Err(_) => continue,
            };
            match key {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    let actions = handle_event(&mut state, key);
                    for action in actions {
                        if let Err(e) = process_action(&mut state, action) {
                            state.status_message = Some(format!("Error: {}", e));
                        }
                    }

                    if let Some(msg) = state.pending_auto_commit.take() {
                        if let Err(e) = git::save(&state.roost_dir, &msg) {
                            state.status_message = Some(format!("Auto-commit failed: {}", e));
                        }
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

/// Execute a single action produced by the event handler.
fn process_action(state: &mut MainViewState, action: Action) -> Result<()> {
    match action {
        Action::Quit => state.quit = true,
        Action::SetStatus(msg) => state.status_message = Some(msg),
        Action::AutoCommit(msg) => state.pending_auto_commit = Some(msg),
        Action::RemoveApp(app_name) => {
            match crate::ops::remove_app(&app_name, &mut state.shared, &mut state.local, &state.roost_dir) {
                Ok(()) => {
                    state.pending_auto_commit = Some(format!("remove: {}", app_name));
                    state.status_message = Some(format!("Removed '{}'", app_name));
                    state.app_cursor = state.app_cursor.min(state.app_count().saturating_sub(1));
                    state.sync_miller_to_selected_app();
                }
                Err(e) => {
                    state.status_message = Some(format!("Error removing {}: {}", app_name, e));
                }
            }
        }
        Action::OpenEditor(path) => {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let result = suspend_and_run(|| {
                std::process::Command::new(&editor).arg(&path).status()?;
                Ok(())
            });
            state.needs_redraw = true;
            if let Err(e) = result {
                state.status_message = Some(format!("Editor error: {}", e));
            }
        }
        Action::OpenPager(content_key) => match content_key.as_str() {
            "git_diff" => {
                let roost_dir = state.roost_dir.clone();
                let result = suspend_and_run(|| {
                    let diff = git::diff(&roost_dir)?;
                    pager::open(&diff)?;
                    Ok(())
                });
                state.needs_redraw = true;
                if let Err(e) = result {
                    state.status_message = Some(format!("Diff error: {}", e));
                }
            }
            other => {
                state.status_message = Some(format!("Unknown pager content: {}", other));
            }
        },
        Action::Sync => {
            let roost_dir = state.roost_dir.clone();
            let commit_msg = match git::diff_stat(&roost_dir) {
                Ok(stat) if !stat.is_empty() => format!("save: {}", stat),
                _ => "save: sync pending changes".to_string(),
            };
            let result = suspend_and_run(|| {
                git::save(&roost_dir, &commit_msg)?;
                let sync_result = git::sync(&roost_dir, crate::git::ConflictPreference::Local)?;
                Ok(sync_result)
            });
            state.needs_redraw = true;
            let mut link_actions: Vec<String> = Vec::new();
            match result {
                Ok(crate::git::SyncResult::Clean)
                | Ok(crate::git::SyncResult::ConfigConflict { .. })
                | Ok(crate::git::SyncResult::FileConflict { .. }) => {
                    // Reload configs from disk since sync may have mutated roost.toml
                    let shared_path = app::shared_config_path(&state.roost_dir);
                    let local_path = app::local_config_path(&state.roost_dir);
                    if let (Ok(shared), Ok(mut local)) =
                        (app::load_shared(&shared_path), app::load_local(&local_path))
                    {
                        if let Ok(actions) =
                            linker::ensure_links(&shared, &mut local, &state.roost_dir)
                        {
                            link_actions = actions;
                            // Best-effort save in case ensure_links auto-discovered link_paths.
                            // The canonical save happens on explicit user save action.
                            let _ = app::save_local(&local_path, &local);
                        }
                        state.reload_configs(shared, local);
                    }
                }
                Err(_) => {}
            }
            match result {
                Ok(crate::git::SyncResult::Clean) => {
                    state.status_message = Some("Sync complete. Pushed to origin.".to_string());
                }
                Ok(crate::git::SyncResult::ConfigConflict { resolved }) => {
                    state.status_message = Some(format!(
                        "Sync complete. {} config conflict(s) resolved. Pushed to origin.",
                        resolved.len()
                    ));
                }
                Ok(crate::git::SyncResult::FileConflict { file_conflicts, .. }) => {
                    let conflict_list = file_conflicts.join(", ");
                    state.status_message = Some(format!(
                        "Sync conflict: {}. Run 'cd ~/.roost && git status', fix conflicts, then sync again.",
                        if conflict_list.is_empty() { "file conflicts with remote".to_string() } else { conflict_list }
                    ));
                }
                Err(e) => {
                    state.status_message = Some(format!("Sync error: {}", e));
                }
            }
            if !link_actions.is_empty() {
                let summary = format!("{} app(s) linked after sync.", link_actions.len());
                state.status_message = Some(match state.status_message {
                    Some(ref msg) => format!("{} {}", msg, summary),
                    None => summary,
                });
            }
        }
        Action::Save => {
            let roost_dir = state.roost_dir.clone();
            if !git::is_dirty(&roost_dir).unwrap_or(false) {
                state.status_message = Some("Nothing to save.".to_string());
            } else {
                let commit_msg = match git::diff_stat(&roost_dir) {
                    Ok(stat) if !stat.is_empty() => format!("save: {}", stat),
                    _ => "save: manual save".to_string(),
                };
                match git::save(&roost_dir, &commit_msg) {
                    Ok(true) => state.status_message = Some("Saved.".to_string()),
                    Ok(false) => state.status_message = Some("Nothing to save.".to_string()),
                    Err(e) => state.status_message = Some(format!("Save failed: {}", e)),
                }
            }
        }
        Action::SwitchProfile(name) => {
            if !state.shared.profiles.contains_key(&name) {
                state.status_message = Some(format!("Profile '{}' does not exist", name));
                return Ok(());
            }
            let old_profile = state.local.active_profile.clone();
            if old_profile == name {
                return Ok(());
            }

            if let Err(e) = crate::ops::switch_profile(
                &old_profile,
                &name,
                &state.shared,
                &mut state.local,
                &state.roost_dir,
            ) {
                state.status_message = Some(format!("Error switching profile: {}", e));
                return Ok(());
            }

            state.app_cursor = 0;
            state.sync_miller_to_selected_app();
            state.status_message = Some(format!("Switched to profile '{}'", name));
        }
        Action::SetPrimary { app, path } => {
            let source: Option<String> = state.selected_app_source().cloned();
            let source_ref = source.as_deref();
            match crate::ops::set_primary(&app, &path, source_ref, &mut state.shared, &state.local, &state.roost_dir) {
                Ok(()) => {
                    state.status_message = Some(format!("Set primary config for '{}'", app));
                }
                Err(e) => {
                    state.status_message = Some(format!("Error setting primary config: {}", e));
                }
            }
        }
        Action::Refresh => {
            // Reload configs
            let shared_path = app::shared_config_path(&state.roost_dir);
            let local_path = app::local_config_path(&state.roost_dir);
            if let Ok(s) = app::load_shared(&shared_path) {
                state.shared = s;
            }
            if let Ok(l) = app::load_local(&local_path) {
                state.local = l;
            }
            state.sync_miller_to_selected_app();
        }
        Action::Nop => {}
        Action::CreateProfile { name, copy_current } => {
            let copy_from = if copy_current {
                Some(state.local.active_profile.as_str())
            } else {
                None
            };
            match crate::ops::create_profile(&name, copy_from, &mut state.shared, &state.local, &state.roost_dir) {
                Ok(()) => {
                    state.status_message = Some(format!("Created profile '{}'", name));
                }
                Err(e) => {
                    state.status_message = Some(format!("Error creating profile: {}", e));
                }
            }
        }
        Action::DeleteProfile(name) => {
            match crate::ops::delete_profile(&name, &mut state.shared, &mut state.local, &state.roost_dir) {
                Ok(()) => {
                    state.status_message = Some(format!("Deleted profile '{}'", name));
                    state.profile_dialog = None;
                }
                Err(e) => {
                    state.status_message = Some(format!("Error deleting profile: {}", e));
                }
            }
        }
        Action::ImportApp {
            app,
            source_profile,
        } => {
            match crate::ops::import_app(&app, &source_profile, &mut state.shared, &mut state.local, &state.roost_dir) {
                Ok(_) => {
                    state.pending_auto_commit = Some(format!("import: {} from {}", app, source_profile));
                    state.status_message =
                        Some(format!("Imported '{}' from '{}'", app, source_profile));
                    state.sync_miller_to_selected_app();
                }
                Err(e) => {
                    state.status_message = Some(format!("Error importing '{}': {}", app, e));
                }
            }
        }
        Action::CopyApp {
            app,
            target_profile,
        } => {
            match crate::ops::copy_app(&app, &target_profile, &mut state.shared, &state.local, &state.roost_dir) {
                Ok(_) => {
                    state.pending_auto_commit = Some(format!("copy: {} to {}", app, target_profile));
                    state.status_message = Some(format!("Copied '{}' to '{}'", app, target_profile));
                    state.sync_miller_to_selected_app();
                }
                Err(e) => {
                    state.status_message = Some(format!("Error copying '{}': {}", app, e));
                }
            }
        }
        Action::SuspendForAddApp => {
            let roost_dir = state.roost_dir.clone();
            let _profile_name = state.local.active_profile.clone();
            let ignored: HashSet<String> = state.shared.ignored.iter().cloned().collect();
            let result = suspend_and_run(|| {
                let home =
                    dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                let sources = crate::scanner::default_scan_sources(&home);
                let scan_items = crate::scanner::scan_sources(&sources, &ignored);
                match crate::app_selector::run_selection_tui(
                    scan_items,
                    &home,
                    &crate::tui::SHOULD_EXIT,
                    false,
                )? {
                    crate::app_selector::TuiResult::Selected(items) => Ok(items),
                    crate::app_selector::TuiResult::Aborted => Ok(Vec::new()),
                }
            });
            state.needs_redraw = true;
            match result {
                Ok(items) if !items.is_empty() => {
                    let mut added = Vec::new();
                    let mut failures = Vec::new();
                    for item in &items {
                        let app_name_raw = if item.name.starts_with('.') && item.name.len() > 1 {
                            &item.name[1..]
                        } else {
                            &item.name
                        };
                        let app_name_raw = if app_name_raw.is_empty() {
                            &item.name
                        } else {
                            app_name_raw
                        };
                        let app_name = app::sanitize_app_name(app_name_raw);
                        let app_name = if app_name.is_empty() {
                            item.name.clone()
                        } else {
                            app_name
                        };
                        let is_dir = item.item_type == crate::scanner::ItemType::Dir;
                        match crate::ops::add_app(&item.path, &app_name, is_dir, &mut state.shared, &mut state.local, &roost_dir) {
                            Ok(result) => {
                                added.push(result.app_name);
                            }
                            Err(e) => {
                                failures.push(format!("'{}': {}", app_name, e));
                            }
                        }
                    }
                    if !added.is_empty() {
                        let names = added.join(", ");
                        state.pending_auto_commit = Some(format!("add: {}", names));
                        state.status_message = Some(format!("Added {} app(s)", added.len()));
                    }
                    if !failures.is_empty() {
                        let msg = format!("Failures: {}", failures.join("; "));
                        state.status_message = Some(msg);
                    }
                    state.sync_miller_to_selected_app();
                }
                Ok(_) => {
                    state.status_message = Some("No apps selected".to_string());
                }
                Err(e) => {
                    state.status_message = Some(format!("Add app failed: {}", e));
                }
            }
        }
        Action::AddIgnore(pattern) => {
            match crate::ops::add_ignore(None, &pattern, &mut state.shared, &state.roost_dir) {
                Ok(true) => {
                    state.status_message = Some(format!("Added ignore pattern '{}'", pattern));
                }
                Ok(false) => {
                    state.status_message = Some(format!("Ignore pattern '{}' already exists", pattern));
                }
                Err(e) => {
                    state.status_message = Some(format!("Error adding ignore pattern: {}", e));
                }
            }
        }
        Action::RemoveIgnore(pattern) => {
            match crate::ops::remove_ignore(None, &pattern, &mut state.shared, &state.roost_dir) {
                Ok(true) => {
                    state.ignore_dialog = None;
                    state.status_message = Some(format!("Removed ignore pattern '{}'", pattern));
                }
                Ok(false) => {
                    state.ignore_dialog = None;
                    state.status_message = Some(format!("Ignore pattern '{}' not found", pattern));
                }
                Err(e) => {
                    state.status_message = Some(format!("Error removing ignore pattern: {}", e));
                }
            }
        }
        Action::Undo => {
            let roost_dir = state.roost_dir.clone();
            let shared = state.shared.clone();
            let local = state.local.clone();
            let profile_name = state.local.active_profile.clone();

            let result = suspend_and_run(|| {
                crate::git::safe_rollback(&roost_dir, "HEAD~1", &shared, &local, &profile_name)
            });

            state.needs_redraw = true;
            match result {
                Ok(()) => {
                    state.status_message =
                        Some("Undone last commit with app preservation".to_string());
                }
                Err(e) => {
                    state.status_message = Some(format!("Undo failed: {}", e));
                }
            }

            let shared_path = crate::app::shared_config_path(&state.roost_dir);
            let local_path = crate::app::local_config_path(&state.roost_dir);
            if let (Ok(shared), Ok(local)) = (
                crate::app::load_shared(&shared_path),
                crate::app::load_local(&local_path),
            ) {
                state.reload_configs(shared, local);
            }
        }
        Action::Rollback(hash) => {
            let roost_dir = state.roost_dir.clone();
            let shared = state.shared.clone();
            let local = state.local.clone();
            let profile_name = state.local.active_profile.clone();

            let result = suspend_and_run(|| {
                crate::git::safe_rollback(&roost_dir, &hash, &shared, &local, &profile_name)
            });

            state.needs_redraw = true;
            match result {
                Ok(()) => {
                    state.status_message = Some(format!(
                        "Rolled back to {} with app preservation",
                        &hash[..hash.len().min(7)]
                    ));
                }
                Err(e) => {
                    state.status_message = Some(format!("Rollback failed: {}", e));
                }
            }

            let shared_path = crate::app::shared_config_path(&state.roost_dir);
            let local_path = crate::app::local_config_path(&state.roost_dir);
            if let (Ok(shared), Ok(local)) = (
                crate::app::load_shared(&shared_path),
                crate::app::load_local(&local_path),
            ) {
                state.reload_configs(shared, local);
            }
        }
    }
    Ok(())
}
