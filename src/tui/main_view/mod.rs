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
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame, Terminal,
};

use crate::app::{self, LocalAppConfig, SharedAppConfig};
use crate::git;
use crate::pager;
use crate::tui::main_view::event::{handle_event, Action};
use crate::tui::main_view::ui::render;
use crate::tui::suspend::suspend_and_run;

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

/// Minimum terminal dimensions for the main TUI.
/// Below these, a "terminal too small" placeholder is shown instead.
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;

/// Launch the main daily-use TUI.
///
/// Sets up the terminal, runs the event loop, processes actions, and restores
/// the terminal on exit.
pub fn run(
    roost_dir: PathBuf,
    shared: SharedAppConfig,
    local: LocalAppConfig,
) -> Result<()> {
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

    let result = run_loop(&mut terminal, roost_dir, shared, local);

    restore_terminal(&mut terminal)?;

    result
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
    let line = Line::from(vec![
        Span::styled(
            message,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(Clear, size);
    frame.render_widget(paragraph, size);
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    roost_dir: PathBuf,
    shared: SharedAppConfig,
    local: LocalAppConfig,
) -> Result<()> {
    let mut state = MainViewState::new(roost_dir, shared, local);

    loop {
        if state.needs_redraw {
            terminal.clear()?;
            state.needs_redraw = false;
        }

        let size = terminal.size()?;
        if size.width < MIN_WIDTH || size.height < MIN_HEIGHT {
            let area = Rect::new(0, 0, size.width, size.height);
            terminal.draw(|f| {
                render_too_small(f, area);
            })?;
        } else {
            terminal.draw(|f| {
                render(&mut state, f);
            })?;
        }

        if SHOULD_EXIT.load(Ordering::Relaxed) {
            state.quit = true;
        }

        if state.quit {
            break;
        }

        if crossterm_poll(std::time::Duration::from_millis(100))? {
            match crossterm_read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    let actions = handle_event(&mut state, key);
                    for action in actions {
                        process_action(&mut state, action)?;
                    }

                    // Batched auto-commit after each event iteration.
                    if let Some(msg) = state.pending_auto_commit.take() {
                        let _ = git::save(&state.roost_dir, &msg);
                    }
                }
                Event::Resize(_, _) => {
                    // Redraw on resize so the too-small check re-evaluates.
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Execute a single action produced by the event handler.
fn process_action(state: &mut MainViewState, action: Action) -> Result<()> {
    match action {
        Action::Quit => state.quit = true,
        Action::SetStatus(msg) => {
            if msg.starts_with("rollback_pending:") {
                // Marker from git log dialog; the confirm dialog handler will pick this up
            } else {
                state.status_message = Some(msg);
            }
        }
        Action::AutoCommit(msg) => state.pending_auto_commit = Some(msg),
        Action::RemoveApp(app) => {
            // TODO: full remove logic in Stream 3/4
            state.status_message = Some(format!("Removed '{}'", app));
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
        Action::OpenPager(content_key) => {
            match content_key.as_str() {
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
            }
        }
        Action::Sync => {
            let roost_dir = state.roost_dir.clone();
            let result = suspend_and_run(|| {
                git::save(&roost_dir, "Auto-commit before sync")?;
                let sync_result = git::sync(&roost_dir, crate::git::ConflictPreference::Local)?;
                Ok(sync_result)
            });
            state.needs_redraw = true;
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
                    state.status_message = Some(
                        "Sync complete with file conflicts. Check status.".to_string(),
                    );
                }
                Err(e) => {
                    state.status_message = Some(format!("Sync error: {}", e));
                }
            }
        }
        Action::SwitchProfile(name) => {
            state.local.active_profile = name;
            state.app_cursor = 0;
            state.sync_miller_to_selected_app();
            let local_path = app::local_config_path(&state.roost_dir);
            let _ = app::save_local(&local_path, &state.local);
            state.status_message = Some(format!("Switched to profile '{}'", state.local.active_profile));
        }
        Action::SetPrimary { app, path } => {
            if let Some(app_entry) = state.shared.apps.get_mut(&app) {
                // Convert internal roost path to original path (symlink target)
                let resolved = if let Some(original_base) = state.local.link_paths.get(&app) {
                    let app_dir = crate::app::profile_dir(&state.roost_dir, &state.local.active_profile)
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
                    let source_profile_dir = crate::app::profile_dir(&state.roost_dir, &state.local.active_profile);
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
                                let _ = std::fs::create_dir_all(target.parent().unwrap_or(&target_profile_dir));
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
        Action::ImportApp { app, source_profile } => {
            state.status_message = Some(format!(
                "Import '{}' from '{}' not yet implemented",
                app, source_profile
            ));
        }
        Action::CopyApp { app, target_profile } => {
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
                let home = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                let sources = crate::scanner::default_scan_sources(&home);
                let scan_items = crate::scanner::scan_sources(&sources, &ignored);
                match crate::app_selector::run_selection_tui(scan_items, &home, &SHOULD_EXIT, false)? {
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
                        let app_name = if app_name.is_empty() { &item.name } else { app_name };
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
                                if let Some(profile) = state.shared.profiles.get_mut(&profile_name) {
                                    profile.apps.insert(app_name.to_string());
                                }
                                state.local.link_paths.insert(app_name.to_string(), item.path.clone());
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
                        let _ = crate::linker::ensure_links(&state.shared, &state.local, &roost_dir);
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
            let _ = crate::gitignore::regenerate(&state.roost_dir, &state.shared.ignored, &state.shared.apps);
            state.status_message = Some(format!("Added ignore pattern '{}'", pattern));
        }
        Action::RemoveIgnore(pattern) => {
            state.shared.ignored.remove(&pattern);
            let shared_path = app::shared_config_path(&state.roost_dir);
            let _ = app::save_shared(&shared_path, &state.shared);
            let _ = crate::gitignore::regenerate(&state.roost_dir, &state.shared.ignored, &state.shared.apps);
            state.status_message = Some(format!("Removed ignore pattern '{}'", pattern));
        }
        Action::Undo => {
            let roost_dir = state.roost_dir.clone();
            let result = suspend_and_run(|| {
                crate::git::undo(&roost_dir, 1)?;
                Ok(())
            });
            state.needs_redraw = true;
            match result {
                Ok(()) => state.status_message = Some("Undone last commit".to_string()),
                Err(e) => state.status_message = Some(format!("Undo failed: {}", e)),
            }
        }
        Action::Rollback(hash) => {
            let roost_dir = state.roost_dir.clone();
            let result = suspend_and_run(|| {
                crate::git::rollback(&roost_dir, &hash)?;
                Ok(())
            });
            state.needs_redraw = true;
            match result {
                Ok(()) => state.status_message = Some(format!("Rolled back to {}", &hash[..hash.len().min(7)])),
                Err(e) => state.status_message = Some(format!("Rollback failed: {}", e)),
            }
        }
    }
    Ok(())
}
