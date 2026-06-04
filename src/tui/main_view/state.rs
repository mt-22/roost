use std::path::PathBuf;

use crate::app::{LocalAppConfig, SharedAppConfig};
use crate::miller::MillerColumns;
use crate::tui::confirm::ConfirmDialog;
use crate::tui::main_view::dialogs::{GitLogState, HelpState, IgnoreState, ProfileState, UndoState};
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
    pub needs_redraw: bool,
    pub quit: bool,
}

/// Active fuzzy-search overlay.
pub struct SearchState {
    pub engine: FuzzyEngine,
    pub query: String,
    pub target: SearchTarget,
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

pub struct AppLinkState;
pub struct DiffViewState;

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
    pub fn sync_miller_to_selected_app(&mut self) {
        if let Some(app_name) = self.selected_app() {
            let app_dir = if let Some(source) = self.selected_app_source() {
                crate::app::profile_dir(&self.roost_dir, source).join(app_name)
            } else {
                crate::app::profile_dir(&self.roost_dir, &self.local.active_profile)
                    .join(app_name)
            };
            self.miller.set_root(&app_dir);
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
}
