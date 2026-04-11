pub mod dialogs;
pub mod event;
pub mod state;
pub mod ui;

use color_eyre;

use crate::app::{LocalAppConfig, SharedAppConfig};

use std::path::PathBuf;

pub fn run_main_view(
    config: SharedAppConfig,
    roost_dir: PathBuf,
    config_path: PathBuf,
    local_config_path: PathBuf,
    local: LocalAppConfig,
) -> color_eyre::Result<()> {
    let mut state =
        state::MainViewTui::new(config, roost_dir, config_path, local_config_path, local)?;
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut state);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut state::MainViewTui,
) -> color_eyre::Result<()> {
    loop {
        terminal.draw(|frame| ui::render(state, frame))?;

        let action = event::handle_event(state)?;

        if let Some(msg) = state.pending_auto_commit.take() {
            let _ = crate::git::auto_commit(&state.roost_dir, &msg);
        }

        match action {
            event::Action::Continue => {}
            event::Action::Quit => return Ok(()),
            event::Action::OpenEditor(path) => {
                open_editor(terminal, &path)?;
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let _ = crate::git::auto_commit(&state.roost_dir, &format!("edited {}", file_name));
            }
            event::Action::Sync => {
                run_sync_flow(terminal, state)?;
            }
            event::Action::AddApp => {
                let prev_count = state.app_count();
                run_add_app_flow(terminal, state)?;
                let added = state.app_count().saturating_sub(prev_count);
                if added > 0 {
                    let _ = crate::git::auto_commit(
                        &state.roost_dir,
                        &format!("added {} app(s)", added),
                    );
                }
            }
            event::Action::RemoveApp => {
                run_remove_app_flow(terminal, state)?;
            }
            event::Action::Diff => {
                run_diff_flow(terminal, state)?;
            }
            event::Action::ShowCommitDiff(hash) => {
                run_commit_diff_flow(terminal, state, &hash)?;
            }
        }
    }
}

fn open_editor(
    terminal: &mut ratatui::DefaultTerminal,
    path: &std::path::Path,
) -> color_eyre::Result<()> {
    ratatui::restore();

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor).arg(path).status()?;

    if !status.success() {
        eprintln!("editor exited with non-zero status");
    }

    *terminal = ratatui::init();
    Ok(())
}

fn run_sync_flow(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut state::MainViewTui,
) -> color_eyre::Result<()> {
    ratatui::restore();

    let dirty = crate::git::is_dirty(&state.roost_dir).unwrap_or(false);
    if dirty {
        if let Ok(diff) = crate::git::diff_text(&state.roost_dir) {
            if !diff.trim().is_empty() {
                let _ = crate::pager::show_in_pager(&diff);
            }
        }
        println!();
        print!("Commit and sync? [y/N] ");
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer).ok();
        if answer.trim().to_lowercase() != "y" {
            *terminal = ratatui::init();
            return Ok(());
        }
    }

    println!();
    match crate::git::sync(&state.roost_dir) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Sync error: {}", e);
        }
    }

    state.reload_config()?;
    crate::linker::ensure_links(&state.config, &state.local, &state.roost_dir);

    println!();
    println!("Press enter to continue...");
    let _ = std::io::stdin().read_line(&mut String::new());

    *terminal = ratatui::init();
    Ok(())
}

fn run_add_app_flow(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut state::MainViewTui,
) -> color_eyre::Result<()> {
    ratatui::restore();

    let existing_app_paths: Vec<PathBuf> = state
        .config
        .apps
        .keys()
        .filter(|name| {
            state
                .config
                .apps
                .get(*name)
                .map(|a| a.on_profiles.contains(&state.active_profile))
                .unwrap_or(false)
        })
        .filter_map(|name| state.local.link_paths.get(name).cloned())
        .collect();

    let ctx = crate::tui::state::OnboardingContext {
        profile_name: state.active_profile.clone(),
        sources: crate::scanner::get_likely_sources(),
        ignored: state.config.ignored.clone(),
        existing_app_paths,
    };

    let selections = crate::tui::run_onboarding(ctx)?;

    // Only skip apps already on the active profile — apps from other profiles
    // can legitimately be added here too.
    let new_selections: Vec<_> = selections
        .into_iter()
        .filter(|entry| {
            !state
                .config
                .apps
                .get(&entry.name)
                .map(|a| a.on_profiles.contains(&state.active_profile))
                .unwrap_or(false)
        })
        .collect();

    if !new_selections.is_empty() {
        let profile_dir = state.roost_dir.join(&state.active_profile);
        println!();
        println!("Linking {} new app(s)...", new_selections.len());

        let mut succeeded: Vec<&crate::scanner::SourceEntry> = Vec::new();
        for entry in &new_selections {
            if let Err(e) = crate::linker::ingest(&entry.path, &profile_dir, &state.roost_dir) {
                eprintln!("  warn: could not ingest {}: {}", entry.name, e);
            } else {
                succeeded.push(entry);
            }
        }

        for entry in &succeeded {
            let app = crate::scanner::entry_to_application(
                entry,
                &state.config.ignored,
                &state.active_profile,
            )?;
            if let Some(profile) = state.config.profiles.get_mut(&state.active_profile) {
                profile.apps.insert(app.name.clone());
            }
            state
                .local
                .link_paths
                .insert(app.name.clone(), entry.path.clone());
            // If the app already exists (managed by other profiles), just add
            // the active profile to on_profiles rather than overwriting the entry.
            if let Some(existing) = state.config.apps.get_mut(&app.name) {
                if !existing.on_profiles.contains(&state.active_profile) {
                    existing.on_profiles.push(state.active_profile.clone());
                }
            } else {
                state.config.apps.insert(app.name.clone(), app);
            }
        }

        state.config.save(&state.config_path)?;
        println!("Config saved.");
    }

    *terminal = ratatui::init();
    state.rebuild_app_list();
    Ok(())
}

fn run_remove_app_flow(
    _terminal: &mut ratatui::DefaultTerminal,
    state: &mut state::MainViewTui,
) -> color_eyre::Result<()> {
    let Some(app_name) = state.selected_app_name().map(|s| s.to_string()) else {
        return Ok(());
    };
    let Some(_app) = state.config.apps.get(&app_name).cloned() else {
        return Ok(());
    };

    let active_profile = state.active_profile.clone();
    let profile_dir = state.roost_dir.join(&active_profile);

    // Unlink only from the active profile
    if let Some(link_path) = state.local.link_paths.get(&app_name) {
        if let Err(e) = crate::linker::unlink(link_path, &profile_dir, &state.roost_dir) {
            eprintln!("  warn: could not unlink: {}", e);
        }
    }
    if let Some(profile) = state.config.profiles.get_mut(&active_profile) {
        profile.apps.remove(&app_name);
    }

    // Remove from config.apps entirely only if no other profile uses it
    if let Some(existing) = state.config.apps.get_mut(&app_name) {
        existing.on_profiles.retain(|p| p != &active_profile);
        if existing.on_profiles.is_empty() {
            state.config.apps.remove(&app_name);
        }
    }

    state.config.save(&state.config_path)?;

    state.rebuild_app_list();
    state.pending_auto_commit = Some(format!("removed app {} from {}", app_name, active_profile));
    Ok(())
}

fn run_diff_flow(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut state::MainViewTui,
) -> color_eyre::Result<()> {
    let diff = state.get_diff()?;
    let Some(diff) = diff else {
        return Ok(());
    };
    ratatui::restore();
    let _ = crate::pager::show_in_pager(&diff);
    *terminal = ratatui::init();
    Ok(())
}

fn run_commit_diff_flow(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut state::MainViewTui,
    hash: &str,
) -> color_eyre::Result<()> {
    let diff = crate::git::diff_for_commit(&state.roost_dir, hash)?;
    ratatui::restore();
    let _ = crate::pager::show_in_pager(&diff);
    *terminal = ratatui::init();
    Ok(())
}
