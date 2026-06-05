/// Add-app dialog — Miller-based file browser for adding apps to roost.
use std::path::PathBuf;

use crate::miller::MillerColumns;
use crate::tui::main_view::state::SearchState;
use crate::tui::search::FuzzyEngine;

/// Which part of the add-app dialog is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddAppFocus {
    Browser,
    NameInput,
}

pub struct AddAppState {
    pub miller: MillerColumns,
    pub search: Option<SearchState>,
    pub name_input: String,
    pub focus: AddAppFocus,
}

impl AddAppState {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let miller = MillerColumns::new(&home);
        Self {
            miller,
            search: None,
            name_input: String::new(),
            focus: AddAppFocus::Browser,
        }
    }

    // --- Browser delegation ---

    pub fn move_up(&mut self) {
        self.miller.move_up();
    }

    pub fn move_down(&mut self) {
        self.miller.move_down();
    }

    pub fn navigate_up(&mut self) {
        self.miller.navigate_up();
    }

    pub fn navigate_down(&mut self) {
        self.miller.navigate_down();
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.miller.current_cursor_path()
    }

    // --- Name input ---

    pub fn push_char(&mut self, c: char) {
        self.name_input.push(c);
    }

    pub fn backspace(&mut self) {
        self.name_input.pop();
    }

    /// Derive a default app name from the selected path.
    pub fn derived_name(&self) -> Option<String> {
        let path = self.selected_path()?;
        let file_name = path.file_name()?.to_string_lossy();
        let name = if file_name.starts_with('.') && file_name.len() > 1 {
            file_name[1..].to_string()
        } else {
            file_name.to_string()
        };
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    /// The effective app name: user input if non-empty, otherwise derived from path.
    pub fn effective_name(&self) -> Option<String> {
        let input = self.name_input.trim();
        if !input.is_empty() {
            Some(input.to_string())
        } else {
            self.derived_name()
        }
    }

    // --- Search ---

    pub fn start_search(&mut self) {
        let mut engine = FuzzyEngine::new();
        let names: Vec<String> = self
            .miller
            .current_entries()
            .iter()
            .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
            .collect();
        engine.filter(&names);
        self.search = Some(SearchState {
            engine,
            query: String::new(),
            target: crate::tui::main_view::state::SearchTarget::Files,
        });
    }

    pub fn clear_search(&mut self) {
        self.search = None;
    }

    pub fn push_search_char(&mut self, c: char) {
        if let Some(ref mut search) = self.search {
            search.query.push(c);
            let names: Vec<String> = self
                .miller
                .current_entries()
                .iter()
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect();
            search.engine.filter(&names);
            if let Some(idx) = search.engine.selected_index() {
                self.miller.set_current_cursor(idx);
            }
        }
    }

    pub fn backspace_search(&mut self) {
        if let Some(ref mut search) = self.search {
            if !search.query.is_empty() {
                search.query.pop();
                let names: Vec<String> = self
                    .miller
                    .current_entries()
                    .iter()
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                    .collect();
                search.engine.filter(&names);
                if let Some(idx) = search.engine.selected_index() {
                    self.miller.set_current_cursor(idx);
                }
            }
        }
    }

    pub fn move_search_up(&mut self) {
        if let Some(ref mut search) = self.search {
            search.engine.move_up();
            if let Some(idx) = search.engine.selected_index() {
                self.miller.set_current_cursor(idx);
            }
        }
    }

    pub fn move_search_down(&mut self) {
        if let Some(ref mut search) = self.search {
            search.engine.move_down();
            if let Some(idx) = search.engine.selected_index() {
                self.miller.set_current_cursor(idx);
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            AddAppFocus::Browser => AddAppFocus::NameInput,
            AddAppFocus::NameInput => AddAppFocus::Browser,
        };
    }
}
