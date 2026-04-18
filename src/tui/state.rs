use crate::scanner::{self, SourceEntry};
use crate::tui::search::SearchState;
use color_eyre::{self, eyre::eyre};
use ratatui::widgets::ListState;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub trait MillerEntry {
    fn path(&self) -> &Path;
    #[allow(dead_code)]
    fn is_dir(&self) -> bool;
}

#[allow(dead_code)]
pub struct OnboardingContext {
    pub profile_name: String,
    pub sources: Vec<PathBuf>,
    pub ignored: HashSet<String>,
    pub existing_app_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Source(usize),
    Browse,
}

#[allow(dead_code)]
pub struct TabState {
    pub label: String,
    pub source: PathBuf,
    pub entries: Vec<SourceEntry>,
    pub list_state: ListState,
}

pub struct MillerState<T: MillerEntry> {
    pub dir_stack: Vec<PathBuf>,
    pub cursors: Vec<usize>,
    pub listings: Vec<Vec<T>>,
}

pub struct OnboardingTui {
    pub active_tab: Tab,
    pub tabs: Vec<TabState>,
    pub miller: MillerState<SourceEntry>,
    pub selected: Vec<SourceEntry>,
    pub context: OnboardingContext,
    pub search: Option<SearchState>,
}

impl TabState {
    fn new(
        source: &Path,
        ignored: &HashSet<String>,
        dotfiles_only: bool,
    ) -> color_eyre::Result<Self> {
        let entries = scanner::scan_source(source, ignored, dotfiles_only)?;
        let mut list_state = ListState::default();
        if !entries.is_empty() {
            list_state.select(Some(0));
        }
        Ok(Self {
            label: scanner::source_label(source),
            source: source.to_path_buf(),
            entries,
            list_state,
        })
    }

    pub fn selected_entry(&self) -> Option<&SourceEntry> {
        self.list_state.selected().and_then(|i| self.entries.get(i))
    }
}

impl<T: MillerEntry> MillerState<T> {
    pub fn new(root: PathBuf, initial_listing: Vec<T>) -> Self {
        Self {
            dir_stack: vec![root],
            cursors: vec![0],
            listings: vec![initial_listing],
        }
    }

    pub fn current_dir(&self) -> &Path {
        self.dir_stack.last().unwrap()
    }

    pub fn current_listing(&self) -> &[T] {
        self.listings.last().map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn current_cursor(&self) -> usize {
        self.cursors.last().copied().unwrap_or(0)
    }

    pub fn current_entry(&self) -> Option<&T> {
        self.current_listing().get(self.current_cursor())
    }

    pub fn parent_listing(&self) -> Option<&[T]> {
        if self.listings.len() >= 2 {
            Some(&self.listings[self.listings.len() - 2])
        } else {
            None
        }
    }

    pub fn parent_cursor(&self) -> Option<usize> {
        if self.cursors.len() >= 2 {
            Some(self.cursors[self.cursors.len() - 2])
        } else {
            None
        }
    }

    pub fn move_down(&mut self) {
        let len = self.current_listing().len();
        if len == 0 {
            return;
        }
        if let Some(cursor) = self.cursors.last_mut() {
            *cursor = (*cursor + 1).min(len - 1);
        }
    }

    pub fn move_up(&mut self) {
        if let Some(cursor) = self.cursors.last_mut() {
            *cursor = cursor.saturating_sub(1);
        }
    }

    pub fn move_right(&mut self, entry: T, child_listing: Vec<T>) {
        self.dir_stack.push(entry.path().to_path_buf());
        self.cursors.push(0);
        self.listings.push(child_listing);
    }

    pub fn move_left(&mut self) {
        if self.dir_stack.len() <= 1 {
            return;
        }
        self.dir_stack.pop();
        self.cursors.pop();
        self.listings.pop();
    }
}

impl OnboardingTui {
    pub fn new(context: OnboardingContext) -> color_eyre::Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| eyre!("could not determine home directory"))?;

        let tabs: Vec<TabState> = context
            .sources
            .iter()
            .filter(|s| s.exists())
            .map(|s| {
                let dotfiles_only = *s == home;
                TabState::new(s, &context.ignored, dotfiles_only)
            })
            .collect::<color_eyre::Result<Vec<_>>>()?;

        let initial_listing =
            scanner::scan_source(&home, &context.ignored, false).unwrap_or_default();
        let miller = MillerState::new(home, initial_listing);

        let active_tab = if !tabs.is_empty() {
            Tab::Source(0)
        } else {
            Tab::Browse
        };

        let existing_set: HashSet<PathBuf> = context.existing_app_paths.iter().cloned().collect();
        let mut selected: Vec<SourceEntry> = Vec::new();

        for tab in &tabs {
            for entry in &tab.entries {
                if existing_set.contains(&entry.path) {
                    selected.push(entry.clone());
                }
            }
        }

        for path in &existing_set {
            if !selected.iter().any(|s| s.path == *path) && path.exists() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                selected.push(SourceEntry {
                    path: path.clone(),
                    name,
                });
            }
        }

        Ok(Self {
            active_tab,
            tabs,
            miller,
            selected,
            context,
            search: None,
        })
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len() + 1
    }

    pub fn tab_labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self.tabs.iter().map(|t| t.label.clone()).collect();
        labels.push("Browse…".to_string());
        labels
    }

    pub fn active_tab_index(&self) -> usize {
        match &self.active_tab {
            Tab::Source(i) => *i,
            Tab::Browse => self.tabs.len(),
        }
    }

    pub fn next_tab(&mut self) {
        let total = self.tab_count();
        let next = (self.active_tab_index() + 1) % total;
        self.active_tab = if next < self.tabs.len() {
            Tab::Source(next)
        } else {
            Tab::Browse
        };
    }

    pub fn prev_tab(&mut self) {
        let total = self.tab_count();
        let idx = self.active_tab_index();
        let prev = if idx == 0 { total - 1 } else { idx - 1 };
        self.active_tab = if prev < self.tabs.len() {
            Tab::Source(prev)
        } else {
            Tab::Browse
        };
    }

    pub fn move_down(&mut self) {
        match &self.active_tab {
            Tab::Source(i) => {
                let tab = &mut self.tabs[*i];
                let len = tab.entries.len();
                if len == 0 {
                    return;
                }
                let next = tab
                    .list_state
                    .selected()
                    .map(|s| (s + 1).min(len - 1))
                    .unwrap_or(0);
                tab.list_state.select(Some(next));
            }
            Tab::Browse => self.miller.move_down(),
        }
    }

    pub fn move_up(&mut self) {
        match &self.active_tab {
            Tab::Source(i) => {
                let tab = &mut self.tabs[*i];
                if tab.entries.is_empty() {
                    return;
                }
                let prev = tab
                    .list_state
                    .selected()
                    .map(|s| s.saturating_sub(1))
                    .unwrap_or(0);
                tab.list_state.select(Some(prev));
            }
            Tab::Browse => self.miller.move_up(),
        }
    }

    pub fn move_left(&mut self) {
        if self.active_tab == Tab::Browse {
            self.miller.move_left();
        }
    }

    pub fn move_right(&mut self) {
        if self.active_tab == Tab::Browse {
            let entry = match self.miller.current_entry() {
                Some(e) if e.path.is_dir() => e.clone(),
                _ => return,
            };
            let listing =
                scanner::scan_source(&entry.path, &self.context.ignored, false).unwrap_or_default();
            self.miller.move_right(entry, listing);
        }
    }

    pub fn toggle_select(&mut self) {
        let entry = match &self.active_tab {
            Tab::Source(i) => self.tabs[*i].selected_entry().cloned(),
            Tab::Browse => self.miller.current_entry().cloned(),
        };

        let Some(entry) = entry else { return };

        if let Some(pos) = self.selected.iter().position(|e| e.path == entry.path) {
            self.selected.remove(pos);
        } else {
            self.selected.push(entry);
        }
    }

    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected.iter().any(|e| e.path == path)
    }

    pub fn start_search(&mut self) {
        let mut search = SearchState::new();
        search.rebuild(&self.current_search_items());
        self.search = Some(search);
    }

    pub fn cancel_search(&mut self) {
        self.search = None;
    }

    pub fn search_accept(&mut self) {
        let Some(ref search) = self.search else {
            return;
        };
        if let Some(idx) = search.selected_index() {
            match &self.active_tab {
                Tab::Source(tab_idx) => {
                    if let Some(tab) = self.tabs.get_mut(*tab_idx) {
                        tab.list_state.select(Some(idx));
                    }
                }
                Tab::Browse => {
                    if idx < self.miller.current_listing().len() {
                        self.miller.cursors.last_mut().map(|c| *c = idx);
                    }
                }
            }
        }
        self.search = None;
    }

    pub fn current_search_items(&self) -> Vec<(String, usize)> {
        match &self.active_tab {
            Tab::Source(tab_idx) => self.tabs[*tab_idx]
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| (e.name.clone(), i))
                .collect(),
            Tab::Browse => self
                .miller
                .current_listing()
                .iter()
                .enumerate()
                .map(|(i, e)| (e.name.clone(), i))
                .collect(),
        }
    }
}
