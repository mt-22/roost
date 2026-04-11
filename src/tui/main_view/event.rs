use color_eyre;
use ratatui::crossterm::event::{self, Event, KeyCode};

use super::state::{ConfirmKind, HelpFocus, IgnoreMode, MainViewTui, PanelFocus, ProfileMode};
use crate::tui::search::{self as search_mod, SearchAction};

pub enum Action {
    Continue,
    Quit,
    OpenEditor(std::path::PathBuf),
    Sync,
    AddApp,
    RemoveApp,
    Diff,
    ShowCommitDiff(String),
}

pub fn handle_event(state: &mut MainViewTui) -> color_eyre::Result<Action> {
    let Event::Key(key) = event::read()? else {
        return Ok(Action::Continue);
    };

    if key.kind != ratatui::crossterm::event::KeyEventKind::Press {
        return Ok(Action::Continue);
    }

    let _had_status = state.status_message.take().is_some();

    if state.undo_confirm.is_some() {
        return handle_undo_confirm(state, key.code);
    }

    if state.confirm_dialog.is_some() {
        return handle_confirm(state, key.code);
    }

    if state.profile_dialog.is_some() {
        return handle_profile(state, key.code);
    }

    if state.git_log_dialog.is_some() {
        return handle_git_log(state, key.code);
    }

    if state.input_dialog.is_some() {
        return handle_input(state, key.code);
    }

    if state.search.is_some() {
        return handle_search(state, key.code);
    }

    if state.app_link_dialog.is_some() {
        return handle_app_link(state, key.code);
    }

    if state.help_dialog.is_some() {
        return handle_help(state, key.code);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(Action::Quit),

        KeyCode::Char('/') => state.start_search(),

        KeyCode::Tab => state.toggle_focus(),

        KeyCode::Char('j') | KeyCode::Down => state.move_down(),
        KeyCode::Char('k') | KeyCode::Up => state.move_up(),

        KeyCode::Char('h') | KeyCode::Left => {
            if state.focus == PanelFocus::Files && state.miller_at_root() {
                state.toggle_focus();
            } else {
                state.move_left();
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if state.focus == PanelFocus::Apps {
                state.toggle_focus();
            } else if let Some(path) = state.try_open_highlighted() {
                return Ok(Action::OpenEditor(path));
            } else {
                state.move_right();
            }
        }

        KeyCode::Char('e') | KeyCode::Enter => {
            if let Some(path) = state.get_file_to_open() {
                return Ok(Action::OpenEditor(path));
            }
        }

        KeyCode::Char('o') => {
            if let Some(path) = state
                .selected_app()
                .and_then(|app| app.primary_config.clone())
            {
                return Ok(Action::OpenEditor(path));
            }
        }

        KeyCode::Char('p') => {
            state.start_set_primary();
        }

        KeyCode::Char('x') => {
            if state.focus == PanelFocus::Apps {
                state.start_remove_app();
            }
        }

        KeyCode::Char('s') => return Ok(Action::Sync),
        KeyCode::Char('a') => return Ok(Action::AddApp),
        KeyCode::Char('i') => state.start_add_ignore(),
        KeyCode::Char('P') => state.start_profile_dialog(),

        KeyCode::Char('g') => {
            state.start_git_log()?;
        }
        KeyCode::Char('d') => return Ok(Action::Diff),
        KeyCode::Char('u') => {
            state.start_undo()?;
        }

        KeyCode::Char('f') if state.focus == PanelFocus::Apps => state.start_link_from(),
        KeyCode::Char('m') if state.focus == PanelFocus::Apps => state.start_paste_into(),

        KeyCode::Char('?') => state.start_help(),

        _ => {}
    }

    Ok(Action::Continue)
}

fn handle_search(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    let items = state.current_search_items();
    let Some(ref mut search_state) = state.search else {
        return Ok(Action::Continue);
    };
    match search_mod::handle_search_key(code, search_state, &items) {
        SearchAction::Cancel => state.cancel_search(),
        SearchAction::Accept => state.search_accept(),
        SearchAction::Continue => {}
    }
    Ok(Action::Continue)
}

fn handle_confirm(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    let is_remove = state
        .confirm_dialog
        .as_ref()
        .is_some_and(|c| matches!(c.kind, ConfirmKind::RemoveApp { .. }));

    match code {
        KeyCode::Char('y') => {
            if is_remove {
                state.confirm_dialog = None;
                return Ok(Action::RemoveApp);
            } else {
                state.confirm_primary()?;
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            state.cancel_primary();
        }
        _ => {}
    }
    Ok(Action::Continue)
}

fn handle_undo_confirm(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    match code {
        KeyCode::Char('y') => {
            state.confirm_undo()?;
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            state.cancel_undo();
        }
        _ => {}
    }
    Ok(Action::Continue)
}

fn handle_profile(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    let has_delete_target = state
        .profile_dialog
        .as_ref()
        .and_then(|pd| pd.delete_target.as_ref())
        .is_some();

    if has_delete_target {
        match code {
            KeyCode::Char('y') => {
                state.profile_confirm_delete()?;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                state.profile_cancel_delete();
            }
            _ => {}
        }
        return Ok(Action::Continue);
    }

    let mode = state
        .profile_dialog
        .as_ref()
        .map(|pd| pd.mode.clone())
        .unwrap_or(ProfileMode::Switch);

    match mode {
        ProfileMode::Switch => match code {
            KeyCode::Esc => state.cancel_profile_dialog(),
            KeyCode::Tab => state.toggle_profile_mode(),
            KeyCode::Char('j') | KeyCode::Down => state.profile_move_down(),
            KeyCode::Char('k') | KeyCode::Up => state.profile_move_up(),
            KeyCode::Enter => {
                state.profile_accept_switch()?;
            }
            _ => {}
        },
        ProfileMode::Create => match code {
            KeyCode::Esc => state.cancel_profile_dialog(),
            KeyCode::Tab => state.toggle_profile_mode(),
            KeyCode::Enter => {
                state.profile_accept_create()?;
            }
            KeyCode::Char(' ') => state.toggle_profile_create_source(),
            KeyCode::Backspace => state.profile_pop(),
            KeyCode::Char(ch) => state.profile_push(ch),
            _ => {}
        },
        ProfileMode::Delete => match code {
            KeyCode::Esc => state.cancel_profile_dialog(),
            KeyCode::Tab => state.toggle_profile_mode(),
            KeyCode::Char('j') | KeyCode::Down => state.profile_move_down(),
            KeyCode::Char('k') | KeyCode::Up => state.profile_move_up(),
            KeyCode::Enter => {
                state.profile_accept_delete();
            }
            _ => {}
        },
    }
    Ok(Action::Continue)
}

fn handle_git_log(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    let has_confirm = state
        .git_log_dialog
        .as_ref()
        .and_then(|d| d.confirm_rollback.as_ref())
        .is_some();

    if has_confirm {
        match code {
            KeyCode::Char('y') => {
                state.git_log_confirm_rollback()?;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                state.git_log_cancel_rollback();
            }
            _ => {}
        }
        return Ok(Action::Continue);
    }

    match code {
        KeyCode::Esc => state.cancel_git_log(),
        KeyCode::Char('j') | KeyCode::Down => state.git_log_move_down(),
        KeyCode::Char('k') | KeyCode::Up => state.git_log_move_up(),
        KeyCode::Enter => {
            if let Some(hash) = state.git_log_accept() {
                return Ok(Action::ShowCommitDiff(hash));
            }
        }
        KeyCode::Char('r') => {
            state.git_log_start_rollback();
        }
        _ => {}
    }
    Ok(Action::Continue)
}

fn handle_app_link(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    match code {
        KeyCode::Esc => state.cancel_app_link(),
        KeyCode::Char('j') | KeyCode::Down => state.app_link_move_down(),
        KeyCode::Char('k') | KeyCode::Up => state.app_link_move_up(),
        KeyCode::Enter => {
            if let Err(e) = state.app_link_accept() {
                state.status_message = Some(format!("Error: {}", e));
            }
        }
        _ => {}
    }
    Ok(Action::Continue)
}

fn handle_help(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    let focus = state
        .help_dialog
        .as_ref()
        .map(|d| d.focus.clone())
        .unwrap_or(HelpFocus::Search);

    match code {
        KeyCode::Esc | KeyCode::Char('?') => {
            state.cancel_help();
        }
        KeyCode::Tab => {
            state.toggle_help_focus();
        }
        KeyCode::Down => state.help_scroll_down(),
        KeyCode::Up => state.help_scroll_up(),
        KeyCode::Char('j') if focus == HelpFocus::List => state.help_scroll_down(),
        KeyCode::Char('k') if focus == HelpFocus::List => state.help_scroll_up(),
        KeyCode::Backspace if focus == HelpFocus::Search => {
            state.help_pop();
        }
        KeyCode::Char(ch) if focus == HelpFocus::Search => state.help_push(ch),
        _ => {}
    }
    Ok(Action::Continue)
}

fn handle_input(state: &mut MainViewTui, code: KeyCode) -> color_eyre::Result<Action> {
    let mode = state
        .input_dialog
        .as_ref()
        .map(|i| i.mode.clone())
        .unwrap_or(IgnoreMode::Add);

    match mode {
        IgnoreMode::Add => match code {
            KeyCode::Esc => state.cancel_input(),
            KeyCode::Tab => state.toggle_ignore_mode(),
            KeyCode::Enter => {
                state.input_accept_add()?;
            }
            KeyCode::Backspace => state.input_pop(),
            KeyCode::Char(ch) => state.input_push(ch),
            _ => {}
        },
        IgnoreMode::Remove => match code {
            KeyCode::Esc => state.cancel_input(),
            KeyCode::Tab => state.toggle_ignore_mode(),
            KeyCode::Char('j') | KeyCode::Down => state.input_move_down(),
            KeyCode::Char('k') | KeyCode::Up => state.input_move_up(),
            KeyCode::Enter => {
                state.input_accept_remove()?;
            }
            KeyCode::Char('/') => state.toggle_ignore_mode(),
            _ => {}
        },
    }
    Ok(Action::Continue)
}
