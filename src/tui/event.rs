use color_eyre;
use ratatui::crossterm::event::{self, Event, KeyCode};

use super::search::{self as search_mod, SearchAction};
use super::state::OnboardingTui;

pub enum Action {
    Continue,
    Finalize,
    Quit,
}

pub fn handle_event(state: &mut OnboardingTui) -> color_eyre::Result<Action> {
    let Event::Key(key) = event::read()? else {
        return Ok(Action::Continue);
    };
    if key.kind != ratatui::crossterm::event::KeyEventKind::Press {
        return Ok(Action::Continue);
    }

    let items = state.current_search_items();
    if let Some(ref mut search_state) = state.search {
        match search_mod::handle_search_key(key.code, search_state, &items) {
            SearchAction::Cancel => state.cancel_search(),
            SearchAction::Accept => state.search_accept(),
            SearchAction::Continue => {}
        }
        return Ok(Action::Continue);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(Action::Quit),
        KeyCode::Char('w') => return Ok(Action::Finalize),
        KeyCode::Char('/') => state.start_search(),
        KeyCode::Tab => state.next_tab(),
        KeyCode::BackTab => state.prev_tab(),
        KeyCode::Char('j') | KeyCode::Down => state.move_down(),
        KeyCode::Char('k') | KeyCode::Up => state.move_up(),
        KeyCode::Char('h') | KeyCode::Left => state.move_left(),
        KeyCode::Char('l') | KeyCode::Right => state.move_right(),
        KeyCode::Char(' ') | KeyCode::Enter => state.toggle_select(),
        _ => {}
    }
    Ok(Action::Continue)
}
