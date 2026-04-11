use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileMode {
    Switch,
    Create,
    Delete,
}

pub struct ProfileDialogState {
    pub mode: ProfileMode,
    pub cursor: usize,
    pub new_name: String,
    pub delete_target: Option<String>,
    /// Whether to clone apps from the current profile (true) or start empty (false).
    pub create_from_current: bool,
}

impl ProfileDialogState {
    pub fn new() -> Self {
        Self {
            mode: ProfileMode::Switch,
            cursor: 0,
            new_name: String::new(),
            delete_target: None,
            create_from_current: true,
        }
    }

    pub fn toggle_create_source(&mut self) {
        self.create_from_current = !self.create_from_current;
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            ProfileMode::Switch => ProfileMode::Create,
            ProfileMode::Create => {
                self.cursor = 0;
                ProfileMode::Delete
            }
            ProfileMode::Delete => {
                self.cursor = 0;
                ProfileMode::Switch
            }
        };
        self.delete_target = None;
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self, count: usize) {
        if count > 0 {
            self.cursor = (self.cursor + 1).min(count - 1);
        }
    }

    pub fn push(&mut self, ch: char) {
        self.new_name.push(ch);
    }

    pub fn pop(&mut self) -> bool {
        self.new_name.pop().is_some()
    }

    pub fn accept_switch(
        &mut self,
        profiles: &HashMap<String, crate::app::Profile>,
    ) -> Option<String> {
        let names = sorted_profile_names(profiles);
        names.get(self.cursor).cloned()
    }

    pub fn accept_create(&self) -> Option<String> {
        let name = self.new_name.trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    pub fn accept_delete(
        &mut self,
        profiles: &HashMap<String, crate::app::Profile>,
        active_profile: &str,
    ) -> Option<String> {
        let names = sorted_profile_names(profiles);
        let Some(name) = names.get(self.cursor).cloned() else {
            return None;
        };
        if name == active_profile {
            return None;
        }
        self.delete_target = Some(name);
        None
    }

    pub fn cancel_delete(&mut self) {
        self.delete_target = None;
    }
}

fn sorted_profile_names(profiles: &HashMap<String, crate::app::Profile>) -> Vec<String> {
    let mut names: Vec<String> = profiles.keys().cloned().collect();
    names.sort();
    names
}
