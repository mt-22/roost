pub mod dialogs;
pub mod event;
pub mod state;
pub mod ui;

pub use state::MainViewState;

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use color_eyre::Result;
use crossterm::event::{Event, KeyEventKind, poll as crossterm_poll, read as crossterm_read};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{self, LocalAppConfig, SharedAppConfig};
use crate::git;
use crate::pager;
use crate::tui::main_view::event::{handle_event, Action};
use crate::tui::main_view::ui::render;
use crate::tui::suspend::suspend_and_run;

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

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
        terminal.draw(|f| {
            render(&mut state, f);
        })?;

        if SHOULD_EXIT.load(Ordering::Relaxed) {
            state.quit = true;
        }

        if state.quit {
            break;
        }

        if crossterm_poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm_read()? {
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
                let _ = git::sync(&roost_dir, crate::git::ConflictPreference::Local);
                Ok(())
            });
            state.needs_redraw = true;
            if let Err(e) = result {
                state.status_message = Some(format!("Sync error: {}", e));
            } else {
                state.status_message = Some("Sync complete".to_string());
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
                app_entry.primary_config = Some(path);
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
