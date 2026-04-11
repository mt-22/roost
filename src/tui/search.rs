use ratatui::crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub struct SearchState {
    pub query: String,
    pub results: Vec<(usize, String)>,
    pub cursor: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            cursor: 0,
        }
    }

    pub fn rebuild(&mut self, items: &[(impl AsRef<str>, usize)]) {
        let query = self.query.to_lowercase();
        self.results = items
            .iter()
            .filter(|(name, _)| fuzzy_match(&name.as_ref().to_lowercase(), &query))
            .map(|(name, idx)| (*idx, name.as_ref().to_string()))
            .collect();
        self.cursor = 0;
    }

    pub fn push(&mut self, ch: char, items: &[(impl AsRef<str>, usize)]) {
        self.query.push(ch);
        self.rebuild(items);
    }

    pub fn pop(&mut self, items: &[(impl AsRef<str>, usize)]) -> bool {
        self.query.pop();
        if self.query.is_empty() {
            false
        } else {
            self.rebuild(items);
            true
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.results.is_empty() {
            self.cursor = (self.cursor + 1).min(self.results.len() - 1);
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.results.get(self.cursor).map(|(idx, _)| *idx)
    }

    pub fn names(&self) -> Vec<&str> {
        self.results.iter().map(|(_, name)| name.as_str()).collect()
    }

    pub fn result_count(&self) -> usize {
        self.results.len()
    }
}

pub enum SearchAction {
    Continue,
    Cancel,
    Accept,
}

pub fn handle_search_key(
    code: KeyCode,
    search: &mut SearchState,
    items: &[(impl AsRef<str>, usize)],
) -> SearchAction {
    match code {
        KeyCode::Esc => SearchAction::Cancel,
        KeyCode::Enter => SearchAction::Accept,
        KeyCode::Up | KeyCode::Char('k') => {
            search.move_up();
            SearchAction::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            search.move_down();
            SearchAction::Continue
        }
        KeyCode::Backspace => {
            if !search.pop(items) {
                SearchAction::Cancel
            } else {
                SearchAction::Continue
            }
        }
        KeyCode::Char(ch) => {
            search.push(ch, items);
            SearchAction::Continue
        }
        _ => SearchAction::Continue,
    }
}

pub fn render_search_overlay(search: &SearchState, frame: &mut Frame) {
    let dialog_width = 40u16;
    let max_visible = 8u16;
    let count = search.result_count() as u16;
    let visible = count.min(max_visible);
    let dialog_height = 2 + visible;

    let area = centered_rect(dialog_width, dialog_height, frame.area());

    frame.render_widget(ratatui::widgets::Clear, area);

    let items: Vec<ListItem> = search
        .names()
        .iter()
        .take(max_visible as usize)
        .enumerate()
        .map(|(i, name)| {
            let style = if i == search.cursor {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(" {}", name)).style(style)
        })
        .collect();

    let title = format!("/{}", search.query);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    let mut list_state = ListState::default();
    if !search.results.is_empty() {
        list_state.select(Some(search.cursor));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

pub fn fuzzy_match(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut chars = query.chars();
    let mut current = chars.next().unwrap();
    for ch in text.chars() {
        if ch == current {
            match chars.next() {
                Some(next) => current = next,
                None => return true,
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_items() -> Vec<(&'static str, usize)> {
        vec![("nvim", 0), ("neovim", 1), ("ghostty", 2)]
    }

    #[test]
    fn test_fuzzy_match_exact() {
        assert!(fuzzy_match("nvim", "nvim"));
    }

    #[test]
    fn test_fuzzy_match_prefix() {
        assert!(fuzzy_match("nvim", "nv"));
    }

    #[test]
    fn test_fuzzy_match_subsequence() {
        assert!(fuzzy_match("nvim", "nvm"));
    }

    #[test]
    fn test_fuzzy_match_out_of_order_fails() {
        assert!(!fuzzy_match("ivm", "nvim"));
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        assert!(fuzzy_match("anything", ""));
    }

    #[test]
    fn test_fuzzy_match_query_longer_than_text_fails() {
        assert!(!fuzzy_match("short", "verylongquery"));
    }

    #[test]
    fn test_fuzzy_match_case_insensitive_in_rebuild() {
        let items: Vec<(&str, usize)> = vec![("NVIM", 0), ("nvim", 1)];
        let mut state = SearchState::new();
        state.query = "nvim".to_string();
        state.rebuild(&items);
        assert_eq!(state.result_count(), 2);
    }

    #[test]
    fn test_search_state_rebuild_filters() {
        let items = test_items();
        let mut state = SearchState::new();
        state.query = "nv".to_string();
        state.rebuild(&items);
        assert!(state.result_count() > 0);
        assert!(state.result_count() < items.len());
    }

    #[test]
    fn test_search_state_push_extends_query() {
        let items = test_items();
        let mut state = SearchState::new();
        state.push('n', &items);
        assert_eq!(state.query, "n");
        state.push('v', &items);
        assert_eq!(state.query, "nv");
    }

    #[test]
    fn test_search_state_pop_removes_last() {
        let items = test_items();
        let mut state = SearchState::new();
        state.query = "nv".to_string();
        let still_active = state.pop(&items);
        assert_eq!(state.query, "n");
        assert!(still_active);
        let still_active = state.pop(&items);
        assert_eq!(state.query, "");
        assert!(!still_active);
    }

    #[test]
    fn test_search_state_move_up_down() {
        let items = test_items();
        let mut state = SearchState::new();
        state.rebuild(&items);
        state.cursor = 1;
        state.move_up();
        assert_eq!(state.cursor, 0);
        state.move_down();
        assert!(state.cursor >= 1);
    }

    #[test]
    fn test_search_state_new_is_empty() {
        let state = SearchState::new();
        assert_eq!(state.result_count(), 0);
        assert!(state.query.is_empty());
        assert_eq!(state.cursor, 0);
    }
}
