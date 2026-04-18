use std::path::{Path, PathBuf};

use crate::app::{LocalAppConfig, SharedAppConfig};
use crate::tui::search::SearchState;
use crate::tui::state::{MillerEntry, MillerState};

use ratatui::widgets::ListState;

pub use super::dialogs::app_link::{AppLinkDialogState, AppLinkMode, LinkFromStep};
pub use super::dialogs::confirm::{ConfirmKind, ConfirmState};
pub use super::dialogs::git_log::GitLogDialogState;
pub use super::dialogs::help::{HelpDialogState, HelpFocus};
pub use super::dialogs::ignore::{IgnoreMode, InputState};
pub use super::dialogs::profile::{ProfileDialogState, ProfileMode};
pub use super::dialogs::undo::UndoConfirmState;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_primary: bool,
}

impl MillerEntry for FileEntry {
    fn path(&self) -> &Path {
        &self.path
    }

    fn is_dir(&self) -> bool {
        self.path.is_dir()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelFocus {
    Apps,
    Files,
}

pub struct MainViewTui {
    pub config: SharedAppConfig,
    pub roost_dir: PathBuf,
    pub config_path: PathBuf,
    pub local_config_path: PathBuf,
    pub local: LocalAppConfig,
    pub active_profile: String,
    pub app_names: Vec<String>,
    pub app_list_state: ListState,
    pub miller: MillerState<FileEntry>,
    pub focus: PanelFocus,
    pub confirm_dialog: Option<ConfirmState>,
    pub search: Option<SearchState>,
    pub input_dialog: Option<InputState>,
    pub profile_dialog: Option<ProfileDialogState>,
    pub git_log_dialog: Option<GitLogDialogState>,
    pub undo_confirm: Option<UndoConfirmState>,
    pub help_dialog: Option<HelpDialogState>,
    pub app_link_dialog: Option<AppLinkDialogState>,
    pub pending_auto_commit: Option<String>,
    pub status_message: Option<String>,
}

impl MainViewTui {
    pub fn new(
        config: SharedAppConfig,
        roost_dir: PathBuf,
        config_path: PathBuf,
        local_config_path: PathBuf,
        mut local: LocalAppConfig,
    ) -> color_eyre::Result<Self> {
        let active_profile = local.active_profile.clone();
        let mut app_names: Vec<String> = config
            .apps
            .iter()
            .filter(|(_, app)| app.on_profiles.contains(&active_profile))
            .map(|(name, _)| name.clone())
            .collect();
        app_names.sort();
        let _ = crate::linker::resolve_missing_link_paths(&active_profile, &config, &mut local);

        let mut app_list_state = ListState::default();
        if !app_names.is_empty() {
            app_list_state.select(Some(0));
        }

        let miller = if !app_names.is_empty() {
            let first_app_name = &app_names[0];
            let primary = config
                .apps
                .get(first_app_name)
                .and_then(|a| a.primary_config.clone());
            if let Some(root) = local.link_paths.get(first_app_name).cloned() {
                let listing = build_tracked_entries(&root, &config.ignored, primary.as_deref());
                MillerState::new(root, listing)
            } else {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
                MillerState::new(home, Vec::new())
            }
        } else {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
            MillerState::new(home, Vec::new())
        };

        Ok(Self {
            config,
            roost_dir,
            config_path,
            local_config_path,
            local,
            active_profile,
            app_names,
            app_list_state,
            miller,
            focus: PanelFocus::Apps,
            confirm_dialog: None,
            search: None,
            input_dialog: None,
            profile_dialog: None,
            git_log_dialog: None,
            undo_confirm: None,
            help_dialog: None,
            app_link_dialog: None,
            pending_auto_commit: None,
            status_message: None,
        })
    }

    pub fn selected_app_name(&self) -> Option<&str> {
        self.app_list_state
            .selected()
            .and_then(|i| self.app_names.get(i))
            .map(|s| s.as_str())
    }

    pub fn selected_app(&self) -> Option<&crate::app::Application> {
        self.selected_app_name()
            .and_then(|name| self.config.apps.get(name))
    }

    pub fn app_count(&self) -> usize {
        self.app_names.len()
    }

    pub fn next_app(&mut self) {
        if self.app_names.is_empty() {
            return;
        }
        let len = self.app_names.len();
        let next = self
            .app_list_state
            .selected()
            .map(|s| (s + 1) % len)
            .unwrap_or(0);
        self.app_list_state.select(Some(next));
        self.rebuild_miller();
    }

    pub fn prev_app(&mut self) {
        if self.app_names.is_empty() {
            return;
        }
        let len = self.app_names.len();
        let prev = self
            .app_list_state
            .selected()
            .map(|s| if s == 0 { len - 1 } else { s - 1 })
            .unwrap_or(0);
        self.app_list_state.select(Some(prev));
        self.rebuild_miller();
    }

    fn rebuild_miller(&mut self) {
        let home = || dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let app_name = self.selected_app_name().map(|s| s.to_string());
        let Some(app_name) = app_name else {
            self.miller = MillerState::new(home(), Vec::new());
            return;
        };
        let Some(root) = self.local.link_paths.get(&app_name).cloned() else {
            self.miller = MillerState::new(home(), Vec::new());
            return;
        };
        let primary = self
            .config
            .apps
            .get(&app_name)
            .and_then(|a| a.primary_config.clone());
        let ignored = self.config.ignored.clone();
        let listing = build_tracked_entries(&root, &ignored, primary.as_deref());
        self.miller = MillerState::new(root, listing);
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Apps => PanelFocus::Files,
            PanelFocus::Files => PanelFocus::Apps,
        };
    }

    pub fn miller_at_root(&self) -> bool {
        self.miller.dir_stack.len() <= 1
    }

    pub fn move_down(&mut self) {
        match self.focus {
            PanelFocus::Apps => {
                self.next_app();
            }
            PanelFocus::Files => {
                self.miller.move_down();
            }
        }
    }

    pub fn move_up(&mut self) {
        match self.focus {
            PanelFocus::Apps => {
                self.prev_app();
            }
            PanelFocus::Files => {
                self.miller.move_up();
            }
        }
    }

    pub fn move_left(&mut self) {
        if self.focus == PanelFocus::Files {
            self.miller.move_left();
        }
    }

    pub fn move_right(&mut self) {
        if self.focus == PanelFocus::Files {
            let entry = match self.miller.current_entry() {
                Some(e) if e.path.is_dir() => e.clone(),
                _ => return,
            };
            let primary = self.selected_app().and_then(|a| a.primary_config.clone());
            let ignored = self.config.ignored.clone();
            let listing = build_tracked_entries(&entry.path, &ignored, primary.as_deref());
            self.miller.move_right(entry, listing);
        }
    }

    pub fn get_file_to_open(&self) -> Option<PathBuf> {
        match self.focus {
            PanelFocus::Files => self
                .miller
                .current_entry()
                .filter(|e| !e.path.is_dir())
                .map(|e| e.path.clone()),
            PanelFocus::Apps => self
                .selected_app()
                .and_then(|app| app.primary_config.clone()),
        }
    }

    pub fn try_open_highlighted(&self) -> Option<PathBuf> {
        if self.focus != PanelFocus::Files {
            return None;
        }
        self.miller
            .current_entry()
            .filter(|e| !e.path.is_dir())
            .map(|e| e.path.clone())
    }

    pub fn start_set_primary(&mut self) {
        if self.focus != PanelFocus::Files {
            return;
        }
        let Some(entry) = self.miller.current_entry() else {
            return;
        };
        if entry.path.is_dir() {
            return;
        }
        let Some(app_name) = self.selected_app_name().map(|s| s.to_string()) else {
            return;
        };
        self.confirm_dialog = Some(ConfirmState::set_primary(entry.path.clone(), app_name));
    }

    pub fn confirm_primary(&mut self) -> color_eyre::Result<bool> {
        let Some(ref mut confirm) = self.confirm_dialog else {
            return Ok(false);
        };
        let app_name = confirm.app_name().to_string();
        let accepted = confirm.accept(&mut self.config, &self.config_path)?;
        if !accepted {
            self.confirm_dialog = None;
            return Ok(false);
        }
        self.confirm_dialog = None;
        self.rebuild_miller();
        self.pending_auto_commit = Some(format!("set primary for {}", app_name));
        Ok(true)
    }

    pub fn cancel_primary(&mut self) {
        self.confirm_dialog = None;
    }

    pub fn start_search(&mut self) {
        let items = self.current_search_items();
        let mut search = SearchState::new();
        search.rebuild(&items);
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
            match self.focus {
                PanelFocus::Apps => {
                    self.app_list_state.select(Some(idx));
                    self.rebuild_miller();
                }
                PanelFocus::Files => {
                    let listing = self.miller.current_listing();
                    if idx < listing.len() {
                        self.miller.cursors.last_mut().map(|c| *c = idx);
                    }
                }
            }
        }
        self.search = None;
    }

    pub fn current_search_items(&self) -> Vec<(String, usize)> {
        match self.focus {
            PanelFocus::Apps => self
                .app_names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.clone(), i))
                .collect(),
            PanelFocus::Files => self
                .miller
                .current_listing()
                .iter()
                .enumerate()
                .map(|(i, e)| (e.name.clone(), i))
                .collect(),
        }
    }

    pub fn reload_config(&mut self) -> color_eyre::Result<()> {
        self.config = SharedAppConfig::load(&self.config_path)?;
        self.rebuild_app_list();
        Ok(())
    }

    pub fn rebuild_app_list(&mut self) {
        self.app_names = self
            .config
            .apps
            .iter()
            .filter(|(_, app)| app.on_profiles.contains(&self.active_profile))
            .map(|(name, _)| name.clone())
            .collect();
        self.app_names.sort();

        if self.app_names.is_empty() {
            self.app_list_state.select(None);
        } else {
            let idx = self
                .app_list_state
                .selected()
                .unwrap_or(0)
                .min(self.app_names.len() - 1);
            self.app_list_state.select(Some(idx));
        }
        self.rebuild_miller();
    }

    pub fn start_add_ignore(&mut self) {
        self.input_dialog = Some(InputState::new_add());
    }

    pub fn cancel_input(&mut self) {
        self.input_dialog = None;
    }

    pub fn toggle_ignore_mode(&mut self) {
        if let Some(ref mut input) = self.input_dialog {
            input.toggle_mode();
        }
    }

    pub fn input_push(&mut self, ch: char) {
        if let Some(ref mut input) = self.input_dialog {
            input.push(ch);
        }
    }

    pub fn input_pop(&mut self) {
        let Some(ref mut input) = self.input_dialog else {
            return;
        };
        if !input.pop() {
            self.input_dialog = None;
        }
    }

    pub fn input_move_up(&mut self) {
        if let Some(ref mut input) = self.input_dialog {
            input.move_up();
        }
    }

    pub fn input_move_down(&mut self) {
        if let Some(ref mut input) = self.input_dialog {
            let count = input.filtered(&self.config.ignored).len();
            input.move_down(count);
        }
    }

    pub fn input_accept_add(&mut self) -> color_eyre::Result<bool> {
        let Some(ref input) = self.input_dialog else {
            return Ok(false);
        };
        let commit_msg = input.accept_add(&mut self.config.ignored);
        self.input_dialog = None;
        if let Some(msg) = commit_msg {
            self.config.save(&self.config_path)?;
            self.pending_auto_commit = Some(msg);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn input_accept_remove(&mut self) -> color_eyre::Result<bool> {
        let Some(ref mut input) = self.input_dialog else {
            return Ok(false);
        };
        let commit_msg = input.accept_remove(&mut self.config.ignored);
        self.input_dialog = None;
        if let Some(msg) = commit_msg {
            self.config.save(&self.config_path)?;
            self.pending_auto_commit = Some(msg);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn filtered_ignores(&self) -> Vec<String> {
        let Some(ref input) = self.input_dialog else {
            return Vec::new();
        };
        input.filtered(&self.config.ignored)
    }

    pub fn start_profile_dialog(&mut self) {
        self.profile_dialog = Some(ProfileDialogState::new());
    }

    pub fn cancel_profile_dialog(&mut self) {
        self.profile_dialog = None;
    }

    pub fn toggle_profile_mode(&mut self) {
        if let Some(ref mut pd) = self.profile_dialog {
            pd.toggle_mode();
        }
    }

    pub fn profile_names_sorted(&self) -> Vec<String> {
        let mut names: Vec<String> = self.config.profiles.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn profile_move_up(&mut self) {
        if let Some(ref mut pd) = self.profile_dialog {
            pd.move_up();
        }
    }

    pub fn profile_move_down(&mut self) {
        let count = self.profile_names_sorted().len();
        if let Some(ref mut pd) = self.profile_dialog {
            pd.move_down(count);
        }
    }

    pub fn profile_push(&mut self, ch: char) {
        if let Some(ref mut pd) = self.profile_dialog {
            pd.push(ch);
        }
    }

    pub fn profile_pop(&mut self) {
        let Some(ref mut pd) = self.profile_dialog else {
            return;
        };
        if !pd.pop() {
            self.profile_dialog = None;
        }
    }

    pub fn profile_accept_switch(&mut self) -> color_eyre::Result<bool> {
        let Some(ref mut pd) = self.profile_dialog else {
            return Ok(false);
        };
        let Some(name) = pd.accept_switch(&self.config.profiles) else {
            return Ok(false);
        };
        let old_profile = self.active_profile.clone();
        crate::linker::resolve_missing_link_paths(&name, &self.config, &mut self.local);
        crate::linker::switch_links(
            &old_profile,
            &name,
            &self.config,
            &self.local,
            &self.roost_dir,
        );
        self.active_profile = name;
        self.local.active_profile = self.active_profile.clone();
        self.local.save(&self.local_config_path)?;

        let reloaded = SharedAppConfig::load(&self.config_path);
        if let Ok(cfg) = reloaded {
            self.config = cfg;
        }
        crate::linker::ensure_links(&self.config, &self.local, &self.roost_dir);

        self.profile_dialog = None;
        self.rebuild_app_list();
        self.pending_auto_commit = Some(format!("switched to profile {}", self.active_profile));
        Ok(true)
    }

    pub fn profile_accept_create(&mut self) -> color_eyre::Result<bool> {
        let Some(ref pd) = self.profile_dialog else {
            return Ok(false);
        };
        let Some(name) = pd.accept_create() else {
            self.profile_dialog = None;
            return Ok(false);
        };
        let from_current = pd.create_from_current;

        let template = self.active_profile.clone();
        let template_arg = if from_current {
            Some(template.as_str())
        } else {
            None
        };
        let count = crate::app::add_profile(
            &name,
            &self.roost_dir,
            &mut self.config,
            &self.config_path,
            &mut self.local,
            &self.local_config_path,
            template_arg,
        )?;

        self.active_profile = name.clone();
        self.local.active_profile = self.active_profile.clone();
        self.local.save(&self.local_config_path)?;
        self.profile_dialog = None;
        self.rebuild_app_list();
        self.pending_auto_commit = Some(format!("created profile {}", name));
        self.status_message = Some(if from_current {
            format!(
                "Created '{}' from '{}' and switched to it. ({} apps)",
                name, template, count
            )
        } else {
            format!("Created and switched to empty profile '{}'.", name)
        });
        Ok(true)
    }

    pub fn toggle_profile_create_source(&mut self) {
        if let Some(ref mut pd) = self.profile_dialog {
            pd.toggle_create_source();
        }
    }

    pub fn profile_accept_delete(&mut self) {
        let Some(ref mut pd) = self.profile_dialog else {
            return;
        };
        pd.accept_delete(&self.config.profiles, &self.active_profile);
    }

    pub fn profile_confirm_delete(&mut self) -> color_eyre::Result<bool> {
        let Some(ref pd) = self.profile_dialog else {
            return Ok(false);
        };
        let Some(ref name) = pd.delete_target else {
            return Ok(false);
        };

        crate::app::delete_profile(
            name,
            &self.roost_dir,
            &mut self.config,
            &self.config_path,
            &mut self.local,
            &self.local_config_path,
        )?;

        let deleted_name = name.clone();
        self.profile_dialog = None;
        self.rebuild_app_list();
        self.pending_auto_commit = Some(format!("deleted profile {}", deleted_name));
        Ok(true)
    }

    pub fn profile_cancel_delete(&mut self) {
        if let Some(ref mut pd) = self.profile_dialog {
            pd.cancel_delete();
        }
    }

    pub fn start_git_log(&mut self) -> color_eyre::Result<()> {
        let entries = crate::git::log(&self.roost_dir, 50)?;
        if entries.is_empty() {
            return Ok(());
        }
        self.git_log_dialog = Some(GitLogDialogState::new(entries));
        Ok(())
    }

    pub fn cancel_git_log(&mut self) {
        self.git_log_dialog = None;
    }

    pub fn git_log_move_up(&mut self) {
        if let Some(ref mut dlg) = self.git_log_dialog {
            dlg.move_up();
        }
    }

    pub fn git_log_move_down(&mut self) {
        if let Some(ref mut dlg) = self.git_log_dialog {
            dlg.move_down();
        }
    }

    pub fn git_log_accept(&self) -> Option<String> {
        self.git_log_dialog
            .as_ref()
            .and_then(|dlg| dlg.selected_hash())
    }

    pub fn git_log_start_rollback(&mut self) {
        if let Some(ref mut dlg) = self.git_log_dialog {
            dlg.start_rollback();
        }
    }

    pub fn git_log_confirm_rollback(&mut self) -> color_eyre::Result<bool> {
        let Some(ref dlg) = self.git_log_dialog else {
            return Ok(false);
        };
        let Some(ref hash) = dlg.confirm_rollback else {
            return Ok(false);
        };
        let hash = hash.clone();
        crate::git::rollback(&self.roost_dir, &hash)?;
        self.git_log_dialog = None;
        self.reload_config()?;
        self.rebuild_app_list();
        Ok(true)
    }

    pub fn git_log_cancel_rollback(&mut self) {
        if let Some(ref mut dlg) = self.git_log_dialog {
            dlg.cancel_rollback();
        }
    }

    pub fn start_undo(&mut self) -> color_eyre::Result<()> {
        let entries = crate::git::log(&self.roost_dir, 1)?;
        let Some(entry) = entries.first() else {
            return Ok(());
        };
        self.undo_confirm = Some(UndoConfirmState::new(entry));
        Ok(())
    }

    pub fn cancel_undo(&mut self) {
        self.undo_confirm = None;
    }

    pub fn confirm_undo(&mut self) -> color_eyre::Result<bool> {
        let Some(ref _uc) = self.undo_confirm else {
            return Ok(false);
        };
        crate::git::undo(&self.roost_dir, 1)?;
        self.undo_confirm = None;
        self.reload_config()?;
        self.rebuild_app_list();
        Ok(true)
    }

    pub fn get_diff(&self) -> color_eyre::Result<Option<String>> {
        if !crate::git::is_dirty(&self.roost_dir)? {
            return Ok(None);
        }
        let diff = crate::git::diff_text(&self.roost_dir)?;
        Ok(Some(diff))
    }

    pub fn start_link_from(&mut self) {
        self.app_link_dialog = Some(AppLinkDialogState::link_from());
    }

    pub fn start_paste_into(&mut self) {
        let Some(app_name) = self.selected_app_name().map(|s| s.to_string()) else {
            return;
        };
        self.app_link_dialog = Some(AppLinkDialogState::paste_into(app_name));
    }

    pub fn cancel_app_link(&mut self) {
        self.app_link_dialog = None;
    }

    pub fn app_link_move_up(&mut self) {
        if let Some(ref mut d) = self.app_link_dialog {
            d.move_up();
        }
    }

    pub fn app_link_move_down(&mut self) {
        let count = match &self.app_link_dialog {
            Some(dlg) => match &dlg.mode {
                AppLinkMode::LinkFrom { step } => match step {
                    LinkFromStep::PickProfile => self.app_link_eligible_profiles().len(),
                    LinkFromStep::PickApp { source_profile } => {
                        self.app_link_eligible_apps(source_profile).len()
                    }
                },
                AppLinkMode::PasteInto { .. } => self.app_link_eligible_profiles().len(),
            },
            None => 0,
        };
        if let Some(ref mut d) = self.app_link_dialog {
            d.move_down(count);
        }
    }

    /// Profiles shown in the LinkFrom step-1 list (have at least one importable app)
    /// or in the PasteInto list (all other profiles).
    pub fn app_link_eligible_profiles(&self) -> Vec<String> {
        let Some(ref dlg) = self.app_link_dialog else {
            return Vec::new();
        };
        let current_prof = self.config.profiles.get(&self.active_profile);
        let mut profiles: Vec<String> = match dlg.mode {
            AppLinkMode::LinkFrom { .. } => self
                .config
                .profiles
                .iter()
                .filter(|(name, prof)| {
                    *name != &self.active_profile
                        && prof.apps.iter().any(|app| {
                            // Real files only (not itself sourced)
                            !prof.app_sources.contains_key(app)
                                // Not already present in the current profile
                                && current_prof
                                    .map(|p| !p.apps.contains(app))
                                    .unwrap_or(true)
                        })
                })
                .map(|(name, _)| name.clone())
                .collect(),
            AppLinkMode::PasteInto { .. } => self
                .config
                .profiles
                .keys()
                .filter(|name| *name != &self.active_profile)
                .cloned()
                .collect(),
        };
        profiles.sort();
        profiles
    }

    /// Apps in `source_profile` that can be imported into the active profile.
    /// Excludes apps already present in the active profile and apps that are
    /// themselves sourced (we want to import from the original only).
    pub fn app_link_eligible_apps(&self, source_profile: &str) -> Vec<String> {
        let Some(source_prof) = self.config.profiles.get(source_profile) else {
            return Vec::new();
        };
        let current_prof = self.config.profiles.get(&self.active_profile);
        let mut apps: Vec<String> = source_prof
            .apps
            .iter()
            .filter(|app| {
                !source_prof.app_sources.contains_key(*app)
                    && current_prof.map(|p| !p.apps.contains(*app)).unwrap_or(true)
            })
            .cloned()
            .collect();
        apps.sort();
        apps
    }

    pub fn app_link_accept(&mut self) -> color_eyre::Result<bool> {
        let Some(ref dlg) = self.app_link_dialog else {
            return Ok(false);
        };

        match dlg.mode.clone() {
            AppLinkMode::LinkFrom { step } => match step {
                LinkFromStep::PickProfile => {
                    let profiles = self.app_link_eligible_profiles();
                    let Some(selected) = dlg.accept(&profiles).cloned() else {
                        return Ok(false);
                    };
                    let apps = self.app_link_eligible_apps(&selected);
                    if apps.is_empty() {
                        self.status_message =
                            Some(format!("No importable apps in '{}'.", selected));
                        self.app_link_dialog = None;
                        return Ok(true);
                    }
                    if let Some(ref mut d) = self.app_link_dialog {
                        d.mode = AppLinkMode::LinkFrom {
                            step: LinkFromStep::PickApp {
                                source_profile: selected,
                            },
                        };
                        d.cursor = 0;
                    }
                    Ok(true)
                }
                LinkFromStep::PickApp { source_profile } => {
                    let apps = self.app_link_eligible_apps(&source_profile);
                    let Some(app_name) = dlg.accept(&apps).cloned() else {
                        return Ok(false);
                    };
                    let active = self.active_profile.clone();
                    self.app_link_dialog = None;

                    crate::linker::import_app_from_profile(
                        &app_name,
                        &active,
                        &source_profile,
                        &mut self.config,
                        &self.config_path,
                        &self.roost_dir,
                        &mut self.local,
                    )?;
                    self.rebuild_app_list();
                    self.pending_auto_commit = Some(format!(
                        "import {} from {} into {}",
                        app_name, source_profile, active
                    ));
                    self.status_message = Some(format!(
                        "'{}' imported from profile '{}'.",
                        app_name, source_profile
                    ));
                    Ok(true)
                }
            },
            AppLinkMode::PasteInto { ref app_name } => {
                let profiles = self.app_link_eligible_profiles();
                let Some(target) = dlg.accept(&profiles).cloned() else {
                    return Ok(false);
                };
                let app_name = app_name.clone();
                let active = self.active_profile.clone();
                self.app_link_dialog = None;

                crate::linker::copy_to_profile(
                    &app_name,
                    &active,
                    &target,
                    &mut self.config,
                    &self.config_path,
                    &self.roost_dir,
                    &self.local.link_paths,
                )?;
                self.rebuild_app_list();
                self.pending_auto_commit =
                    Some(format!("copied {} to profile {}", app_name, target));
                self.status_message =
                    Some(format!("'{}' copied to profile '{}'.", app_name, target));
                Ok(true)
            }
        }
    }

    pub fn start_remove_app(&mut self) {
        let Some(app_name) = self.selected_app_name().map(|s| s.to_string()) else {
            return;
        };
        self.confirm_dialog = Some(ConfirmState::remove_app(app_name));
    }

    pub fn start_help(&mut self) {
        self.help_dialog = Some(HelpDialogState::new());
    }

    pub fn cancel_help(&mut self) {
        self.help_dialog = None;
    }

    pub fn help_push(&mut self, ch: char) {
        if let Some(ref mut dlg) = self.help_dialog {
            dlg.push(ch);
        }
    }

    pub fn help_pop(&mut self) -> bool {
        if let Some(ref mut dlg) = self.help_dialog {
            if !dlg.pop() {
                self.help_dialog = None;
            }
            true
        } else {
            false
        }
    }

    pub fn help_scroll_down(&mut self) {
        if let Some(ref mut dlg) = self.help_dialog {
            let count = dlg.matches().len();
            dlg.scroll_down(count);
        }
    }

    pub fn help_scroll_up(&mut self) {
        if let Some(ref mut dlg) = self.help_dialog {
            dlg.scroll_up();
        }
    }

    pub fn toggle_help_focus(&mut self) {
        if let Some(ref mut dlg) = self.help_dialog {
            dlg.toggle_focus();
        }
    }
}

fn is_primary_in_subdir(
    primary_config: Option<&Path>,
    dir: &Path,
    subdir_name: &std::ffi::OsStr,
) -> bool {
    let Some(p) = primary_config else {
        return false;
    };
    let Ok(rel) = p.strip_prefix(dir) else {
        return false;
    };
    rel.components()
        .next()
        .is_some_and(|c| c.as_os_str() == subdir_name)
}

/// Scan `dir` and return one FileEntry per immediate child, respecting ignores.
/// Directories are collapsed to a single entry; `is_primary` is set if the
/// primary config lives inside that directory.
pub fn build_tracked_entries(
    dir: &Path,
    ignored: &std::collections::HashSet<String>,
    primary_config: Option<&Path>,
) -> Vec<FileEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut entries: Vec<FileEntry> = rd
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if crate::scanner::is_ignored(&name, ignored) {
                return None;
            }
            let path = entry.path();
            let is_primary = if path.is_dir() {
                is_primary_in_subdir(primary_config, dir, entry.file_name().as_os_str())
            } else {
                primary_config.is_some_and(|p| path == p)
            };
            Some(FileEntry {
                path,
                name,
                is_primary,
            })
        })
        .collect();

    entries.sort_by(|a, b| {
        b.path
            .is_dir()
            .cmp(&a.path.is_dir())
            .then(a.name.cmp(&b.name))
    });

    entries
}
