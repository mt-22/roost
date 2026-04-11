use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IgnoreMode {
    Add,
    Remove,
}

pub struct InputState {
    pub query: String,
    pub cursor: usize,
    pub mode: IgnoreMode,
}

impl InputState {
    pub fn new_add() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            mode: IgnoreMode::Add,
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            IgnoreMode::Add => {
                self.cursor = 0;
                IgnoreMode::Remove
            }
            IgnoreMode::Remove => IgnoreMode::Add,
        };
    }

    pub fn push(&mut self, ch: char) {
        self.query.push(ch);
        self.cursor = 0;
    }

    pub fn pop(&mut self) -> bool {
        if self.query.pop().is_none() {
            false
        } else {
            self.cursor = 0;
            true
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self, total: usize) {
        if total > 0 {
            self.cursor = (self.cursor + 1).min(total - 1);
        }
    }

    pub fn accept_add(&self, ignored: &mut HashSet<String>) -> Option<String> {
        let pattern = self.query.trim().to_string();
        if pattern.is_empty() {
            return None;
        }
        ignored.insert(pattern.clone());
        Some(format!("added ignore pattern: {}", pattern))
    }

    pub fn accept_remove(&mut self, ignored: &mut HashSet<String>) -> Option<String> {
        let query = self.query.to_lowercase();
        let filtered = filter_ignores(ignored, &query);
        let Some(pattern) = filtered.get(self.cursor).cloned() else {
            return None;
        };
        ignored.remove(&pattern);
        let new_count = filter_ignores(ignored, &self.query.to_lowercase()).len();
        if new_count == 0 {
            self.cursor = 0;
        } else if self.cursor >= new_count {
            self.cursor = new_count - 1;
        }
        Some(format!("removed ignore pattern: {}", pattern))
    }

    pub fn filtered<'a>(&self, ignored: &'a HashSet<String>) -> Vec<String> {
        filter_ignores(ignored, &self.query.to_lowercase())
    }
}

pub fn filter_ignores(ignored: &HashSet<String>, query: &str) -> Vec<String> {
    let mut sorted: Vec<String> = ignored.iter().cloned().collect();
    sorted.sort();
    if query.is_empty() {
        sorted
    } else {
        sorted
            .into_iter()
            .filter(|p| crate::tui::search::fuzzy_match(&p.to_lowercase(), query))
            .collect()
    }
}
