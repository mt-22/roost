pub mod dialogs;
pub mod event;
pub mod state;
pub mod ui;

pub use state::MainViewState;

use std::collections::HashSet;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

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

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

/// Minimum terminal dimensions for the main TUI.
/// Below these, a "terminal too small" placeholder is shown instead.
const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 12;

/// Launch the main daily-use TUI.
///
/// Sets up the terminal, runs the event loop, processes actions, and restores
/// the terminal on exit.
pub fn run(roost_dir: PathBuf, shared: SharedAppConfig, local: LocalAppConfig) -> Result<()> {
    let mut terminal = setup_terminal()?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
        original_hook(info);
    }));

    SHOULD_EXIT.store(false, Ordering::SeqCst);

    ctrlc::set_handler(|| {
        SHOULD_EXIT.store(true, Ordering::SeqCst);
    })?;

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
            let _ = terminal.clear();
            state.needs_redraw = false;
        }

        let size = match terminal.size() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if size.width < MIN_WIDTH || size.height < MIN_HEIGHT {
            let area = Rect::new(0, 0, size.width, size.height);
            let _ = terminal.draw(|f| {
                render_too_small(f, area);
            });
        } else {
            let _ = terminal.draw(|f| {
                render(&mut state, f);
            });
        }

        if SHOULD_EXIT.load(Ordering::Relaxed) {
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
            let Some(app) = state.shared.apps.get(&app_name) else {
                state.status_message = Some(format!("App '{}' not found", app_name));
                return Ok(());
            };
            let Some(origin) = state.local.link_paths.get(&app_name) else {
                state.status_message = Some(format!("No link path for '{}'", app_name));
                return Ok(());
            };
            let profile_name = state.local.active_profile.clone();
            let pdir = app::profile_dir(&state.roost_dir, &profile_name);

            if let Err(e) = linker::unlink(origin, &pdir, &app_name, app.is_dir) {
                state.status_message = Some(format!("Error removing {}: {}", app_name, e));
                return Ok(());
            }

            state.shared.apps.remove(&app_name);
            if let Some(profile) = state.shared.profiles.get_mut(&profile_name) {
                profile.apps.remove(&app_name);
            }
            state.local.link_paths.remove(&app_name);

            if let Err(e) = app::save_shared(
                &app::shared_config_path(&state.roost_dir),
                &state.shared,
            ) {
                state.status_message = Some(format!("Error saving config: {}", e));
                return Ok(());
            }
            if let Err(e) = app::save_local(
                &app::local_config_path(&state.roost_dir),
                &state.local,
            ) {
                state.status_message = Some(format!("Error saving local config: {}", e));
                return Ok(());
            }

            state.pending_auto_commit = Some(format!("remove: {}", app_name));
            state.status_message = Some(format!("Removed '{}'", app_name));
            state.app_cursor = state
                .app_cursor
                .min(state.app_count().saturating_sub(1));
            state.sync_miller_to_selected_app();
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
                            // Save local.toml in case ensure_links auto-discovered link_paths
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
                Ok(crate::git::SyncResult::FileConflict { .. }) => {
                    state.status_message =
                        Some("Sync complete with file conflicts. Check status.".to_string());
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
            state.local.active_profile = name;
            state.app_cursor = 0;
            state.sync_miller_to_selected_app();
            let local_path = app::local_config_path(&state.roost_dir);
            let _ = app::save_local(&local_path, &state.local);
            state.status_message = Some(format!(
                "Switched to profile '{}'",
                state.local.active_profile
            ));
        }
        Action::SetPrimary { app, path } => {
            if let Some(app_entry) = state.shared.apps.get_mut(&app) {
                // Convert internal roost path to original path (symlink target)
                let resolved = if let Some(original_base) = state.local.link_paths.get(&app) {
                    let app_dir =
                        crate::app::profile_dir(&state.roost_dir, &state.local.active_profile)
                            .join(&app);
                    if path.starts_with(&app_dir) {
                        if let Ok(rel) = path.strip_prefix(&app_dir) {
                            original_base.join(rel)
                        } else {
                            path
                        }
                    } else {
                        path
                    }
                } else {
                    path
                };
                app_entry.primary_config = Some(resolved);
                let shared_path = app::shared_config_path(&state.roost_dir);
                let _ = app::save_shared(&shared_path, &state.shared);
                state.status_message = Some(format!("Set primary config for '{}'", app));
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
            let mut profile = crate::app::Profile {
                apps: std::collections::BTreeSet::new(),
                app_sources: std::collections::BTreeMap::new(),
            };
            if copy_current {
                if let Some(current) = state.shared.profiles.get(&state.local.active_profile) {
                    profile.apps = current.apps.clone();
                    profile.app_sources = current.app_sources.clone();

                    // Physically copy app files into the new profile directory.
                    let source_profile_dir =
                        crate::app::profile_dir(&state.roost_dir, &state.local.active_profile);
                    let target_profile_dir = crate::app::profile_dir(&state.roost_dir, &name);
                    let _ = std::fs::create_dir_all(&target_profile_dir);

                    for app_name in &current.apps {
                        let is_dir = state
                            .shared
                            .apps
                            .get(app_name)
                            .map(|a| a.is_dir)
                            .unwrap_or(true);
                        let source = crate::linker::app_dest(&source_profile_dir, app_name, is_dir);
                        let target = crate::linker::app_dest(&target_profile_dir, app_name, is_dir);
                        if source.exists() && !target.exists() {
                            if is_dir {
                                let _ = crate::linker::copy_dir_recursive(&source, &target);
                            } else {
                                let _ = std::fs::create_dir_all(
                                    target.parent().unwrap_or(&target_profile_dir),
                                );
                                let _ = std::fs::copy(&source, &target);
                            }
                        }
                    }
                }
            }
            state.shared.profiles.insert(name.clone(), profile);
            let shared_path = app::shared_config_path(&state.roost_dir);
            let _ = app::save_shared(&shared_path, &state.shared);
            state.status_message = Some(format!("Created profile '{}'", name));
        }
        Action::DeleteProfile(name) => {
            state.shared.profiles.remove(&name);
            let shared_path = app::shared_config_path(&state.roost_dir);
            let _ = app::save_shared(&shared_path, &state.shared);
            state.status_message = Some(format!("Deleted profile '{}'", name));
        }
        Action::ImportApp {
            app,
            source_profile,
        } => {
            state.status_message = Some(format!(
                "Import '{}' from '{}' not yet implemented",
                app, source_profile
            ));
        }
        Action::CopyApp {
            app,
            target_profile,
        } => {
            state.status_message = Some(format!(
                "Copy '{}' to '{}' not yet implemented",
                app, target_profile
            ));
        }
        Action::SuspendForAddApp => {
            let roost_dir = state.roost_dir.clone();
            let profile_name = state.local.active_profile.clone();
            let ignored: HashSet<String> = state.shared.ignored.iter().cloned().collect();
            let result = suspend_and_run(|| {
                let home =
                    dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                let sources = crate::scanner::default_scan_sources(&home);
                let scan_items = crate::scanner::scan_sources(&sources, &ignored);
                match crate::app_selector::run_selection_tui(
                    scan_items,
                    &home,
                    &SHOULD_EXIT,
                    false,
                )? {
                    crate::app_selector::TuiResult::Selected(items) => Ok(items),
                    crate::app_selector::TuiResult::Aborted => Ok(Vec::new()),
                }
            });
            state.needs_redraw = true;
            match result {
                Ok(items) if !items.is_empty() => {
                    let pdir = crate::app::profile_dir(&roost_dir, &profile_name);
                    let mut added = Vec::new();
                    let mut failures = Vec::new();
                    for item in &items {
                        let app_name = if item.name.starts_with('.') && item.name.len() > 1 {
                            &item.name[1..]
                        } else {
                            &item.name
                        };
                        let app_name = if app_name.is_empty() {
                            &item.name
                        } else {
                            app_name
                        };
                        if state.shared.apps.contains_key(app_name) {
                            failures.push(format!("'{}' already managed", app_name));
                            continue;
                        }
                        let is_dir = item.item_type == crate::scanner::ItemType::Dir;
                        match crate::linker::ingest(&item.path, &pdir, app_name, is_dir) {
                            Ok(()) => {
                                state.shared.apps.insert(
                                    app_name.to_string(),
                                    crate::app::Application {
                                        primary_config: None,
                                        on_profiles: {
                                            let mut s = std::collections::BTreeSet::new();
                                            s.insert(profile_name.clone());
                                            s
                                        },
                                        is_dir,
                                        ignore: Vec::new(),
                                    },
                                );
                                if let Some(profile) = state.shared.profiles.get_mut(&profile_name)
                                {
                                    profile.apps.insert(app_name.to_string());
                                }
                                state
                                    .local
                                    .link_paths
                                    .insert(app_name.to_string(), item.path.clone());
                                added.push(app_name.to_string());
                            }
                            Err(e) => {
                                failures.push(format!("'{}': {}", app_name, e));
                            }
                        }
                    }
                    if !added.is_empty() {
                        let _ = crate::app::guess_primary_configs(
                            &roost_dir,
                            &profile_name,
                            &mut state.shared,
                            &state.local,
                        );
                        let shared_path = crate::app::shared_config_path(&roost_dir);
                        let local_path = crate::app::local_config_path(&roost_dir);
                        let _ = crate::app::save_shared(&shared_path, &state.shared);
                        let _ = crate::app::save_local(&local_path, &state.local);
                        let _ = crate::gitignore::regenerate(
                            &roost_dir,
                            &state.shared.ignored,
                            &state.shared.apps,
                        );
                        let _ = crate::linker::ensure_links(
                            &state.shared,
                            &mut state.local,
                            &roost_dir,
                        );
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
            state.shared.ignored.insert(pattern.clone());
            let shared_path = app::shared_config_path(&state.roost_dir);
            let _ = app::save_shared(&shared_path, &state.shared);
            let _ = crate::gitignore::regenerate(
                &state.roost_dir,
                &state.shared.ignored,
                &state.shared.apps,
            );
            state.status_message = Some(format!("Added ignore pattern '{}'", pattern));
        }
        Action::RemoveIgnore(pattern) => {
            state.shared.ignored.remove(&pattern);
            let shared_path = app::shared_config_path(&state.roost_dir);
            let _ = app::save_shared(&shared_path, &state.shared);
            let _ = crate::gitignore::regenerate(
                &state.roost_dir,
                &state.shared.ignored,
                &state.shared.apps,
            );
            state.status_message = Some(format!("Removed ignore pattern '{}'", pattern));
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
