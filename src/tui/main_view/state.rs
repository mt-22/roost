use std::path::PathBuf;

use crate::app::{LocalAppConfig, SharedAppConfig};
use crate::miller::MillerColumns;
use crate::tui::confirm::ConfirmDialog;
use crate::tui::main_view::dialogs::{AppLinkState, DiffViewState, GitLogState, HelpState, IgnoreState, ProfileState, UndoState};
use crate::tui::search::FuzzyEngine;

/// Which panel currently receives keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    AppsPanel,
    FilesPanel,
}

impl Focus {
    /// Toggle between the two panels.
    pub fn toggle(&self) -> Self {
        match self {
            Focus::AppsPanel => Focus::FilesPanel,
            Focus::FilesPanel => Focus::AppsPanel,
        }
    }
}

/// Top-level mutable state for the main daily-use TUI.
pub struct MainViewState {
    pub roost_dir: PathBuf,
    pub shared: SharedAppConfig,
    pub local: LocalAppConfig,

    // Panels
    pub focus: Focus,
    pub app_cursor: usize,
    pub app_scroll: usize,
    pub miller: MillerColumns,

    // Dialog overlays (flat Option priority stack)
    pub confirm_dialog: Option<ConfirmDialog>,
    pub search: Option<SearchState>,
    pub help_dialog: Option<HelpState>,
    pub profile_dialog: Option<ProfileState>,
    pub ignore_dialog: Option<IgnoreState>,
    pub git_log_dialog: Option<GitLogState>,
    pub undo_dialog: Option<UndoState>,
    pub app_link_dialog: Option<AppLinkState>,
    pub diff_view: Option<DiffViewState>,

    // Meta
    pub status_message: Option<String>,
    pub pending_auto_commit: Option<String>,
    pub pending_action: Option<crate::tui::main_view::event::Action>,
    pub needs_redraw: bool,
    pub quit: bool,
}

/// Active fuzzy-search overlay.
pub struct SearchState {
    pub engine: FuzzyEngine,
    pub target: SearchTarget,
    pub visible: bool,
}

/// What domain the current search operates on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTarget {
    Apps,
    Files,
}

// ------------------------------------------------------------------
// Placeholder dialog states — fleshed out in Stream 3 (Dialog System)
// ------------------------------------------------------------------



impl MainViewState {
    /// Build initial state. The Miller columns start at the active profile root.
    pub fn new(roost_dir: PathBuf, shared: SharedAppConfig, local: LocalAppConfig) -> Self {
        let profile_dir = crate::app::profile_dir(&roost_dir, &local.active_profile);
        let miller = MillerColumns::new(&profile_dir);

        let mut state = Self {
            roost_dir,
            shared,
            local,
            focus: Focus::AppsPanel,
            app_cursor: 0,
            app_scroll: 0,
            miller,
            confirm_dialog: None,
            search: None,
            help_dialog: None,
            profile_dialog: None,
            ignore_dialog: None,
            git_log_dialog: None,
            undo_dialog: None,
            app_link_dialog: None,
            diff_view: None,
            status_message: None,
            pending_auto_commit: None,
            pending_action: None,
            needs_redraw: false,
            quit: false,
        };

        state.sync_miller_to_selected_app();
        state
    }

    // ------------------------------------------------------------------
    // Profile / app introspection
    // ------------------------------------------------------------------

    /// Name of the currently active profile.
    pub fn active_profile_name(&self) -> &str {
        &self.local.active_profile
    }

    /// Number of apps assigned to the active profile.
    pub fn app_count(&self) -> usize {
        self.shared
            .profiles
            .get(&self.local.active_profile)
            .map(|p| p.apps.len())
            .unwrap_or(0)
    }

    /// All app names in the active profile, deterministically sorted.
    pub fn apps_in_active_profile(&self) -> Vec<&String> {
        let mut apps: Vec<&String> = self
            .shared
            .profiles
            .get(&self.local.active_profile)
            .map(|p| p.apps.iter().collect())
            .unwrap_or_default();
        apps.sort();
        apps
    }

    /// The app name currently under the cursor in the left panel, if any.
    pub fn selected_app(&self) -> Option<&String> {
        let apps = self.apps_in_active_profile();
        apps.get(self.app_cursor).copied()
    }

    /// If the selected app is cross-profile linked, returns the source profile name.
    pub fn selected_app_source(&self) -> Option<&String> {
        let app = self.selected_app()?;
        self.shared
            .profiles
            .get(&self.local.active_profile)?
            .app_sources
            .get(app)
    }

    /// Whether the given app has a primary_config registered in `shared.apps`.
    pub fn has_primary_config(&self, app_name: &str) -> bool {
        self.shared
            .apps
            .get(app_name)
            .and_then(|app| app.primary_config.as_ref())
            .is_some()
    }

    /// Re-root the Miller columns to the directory of the currently selected app.
    /// For cross-profile linked apps this points at the source profile's copy.
    /// If the app has a primary_config, navigates to it and highlights it.
    pub fn sync_miller_to_selected_app(&mut self) {
        let Some(app_name) = self.selected_app().cloned() else { return; };
        let app_dir = if let Some(source) = self.selected_app_source() {
            crate::app::profile_dir(&self.roost_dir, source).join(&app_name)
        } else {
            crate::app::profile_dir(&self.roost_dir, &self.local.active_profile)
                .join(&app_name)
        };
        self.miller.set_root(&app_dir);

        // Set primary config highlight and navigate cursor to it
        if let Some(primary) = self.shared.apps.get(&app_name).and_then(|a| a.primary_config.clone()) {
            // Convert original path to internal roost path for miller comparison
            let internal_primary = if let Some(original_base) = self.local.link_paths.get(&app_name) {
                if primary.starts_with(original_base) {
                    if let Ok(rel) = primary.strip_prefix(original_base) {
                        app_dir.join(rel)
                    } else {
                        primary.clone()
                    }
                } else {
                    primary.clone()
                }
            } else {
                primary.clone()
            };
            self.miller.set_primary_config(Some(internal_primary.clone()));

            // Navigate to the primary config if it's within subdirectories
            if internal_primary.starts_with(&app_dir) {
                if let Ok(rel) = internal_primary.strip_prefix(&app_dir) {
                    for component in rel.parent().unwrap_or(std::path::Path::new("")).components() {
                        let comp_str = component.as_os_str().to_string_lossy();
                        let entries = self.miller.current_entries();
                        if let Some(idx) = entries.iter().position(|e| {
                            e.file_name().to_string_lossy() == comp_str
                                && e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        }) {
                            self.miller.set_current_cursor(idx);
                            self.miller.navigate_down();
                        }
                    }
                    // Finally, focus on the file itself
                    let file_name = rel.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                    let entries = self.miller.current_entries();
                    if let Some(idx) = entries.iter().position(|e| e.file_name().to_string_lossy() == file_name) {
                        self.miller.set_current_cursor(idx);
                    }
                }
            }
        } else {
            self.miller.set_primary_config(None);
        }
    }

    // ------------------------------------------------------------------
    // Cursor helpers
    // ------------------------------------------------------------------

    /// Move app cursor down, clamping at the end of the list.
    pub fn app_cursor_down(&mut self) {
        let count = self.app_count();
        if count > 0 && self.app_cursor + 1 < count {
            self.app_cursor += 1;
            self.sync_miller_to_selected_app();
        }
    }

    /// Move app cursor up, clamping at zero.
    pub fn app_cursor_up(&mut self) {
        if self.app_cursor > 0 {
            self.app_cursor -= 1;
            self.sync_miller_to_selected_app();
        }
    }

    /// Scroll helpers for the app list when it exceeds panel height.
    pub fn scroll_for_visible(&self, visible: usize) -> (usize, usize) {
        let total = self.app_count();
        if total == 0 {
            return (0, 0);
        }
        let scroll = if self.app_cursor >= visible {
            self.app_cursor - visible + 1
        } else {
            0
        };
        let end = (scroll + visible).min(total);
        (scroll, end)
    }

    /// Replace shared/local configs and rebuild Miller columns for the active profile.
    /// Used after operations (e.g. sync) that may have mutated roost.toml on disk.
    pub fn reload_configs(&mut self, shared: SharedAppConfig, local: LocalAppConfig) {
        self.shared = shared;
        self.local = local;
        let profile_dir = crate::app::profile_dir(&self.roost_dir, &self.local.active_profile);
        self.miller = MillerColumns::new(&profile_dir);
        self.app_cursor = self.app_cursor.min(self.app_count().saturating_sub(1));
        self.sync_miller_to_selected_app();
    }

    /// Whether a search filter is currently active (overlay may be hidden).
    pub fn is_search_active(&self) -> bool {
        self.search.is_some()
    }

    /// Mutable access to the active search engine, if any.
    pub fn search_engine_mut(&mut self) -> Option<&mut FuzzyEngine> {
        self.search.as_mut().map(|s| &mut s.engine)
    }

    /// Target of the active search, if any.
    pub fn search_target(&self) -> Option<SearchTarget> {
        self.search.as_ref().map(|s| s.target)
    }

    /// Clear any active search filter.
    pub fn clear_search(&mut self) {
        self.search = None;
        self.miller.clear_filter();
    }
}
