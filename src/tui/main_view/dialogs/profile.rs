/// Profile management dialog with three modes (Tab to cycle):
/// 1. Switch — list profiles, pick one to activate
/// 2. Create — type name, choose [current]/[empty]
/// 3. Delete — select profile, confirm deletion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMode {
    Switch,
    Create,
    Delete,
}

impl ProfileMode {
    pub fn next(&self) -> Self {
        match self {
            ProfileMode::Switch => ProfileMode::Create,
            ProfileMode::Create => ProfileMode::Delete,
            ProfileMode::Delete => ProfileMode::Switch,
        }
    }
}

pub struct ProfileState {
    pub mode: ProfileMode,
    pub cursor: usize,
    pub scroll: usize,
    pub input: String,
    pub copy_current: bool,
}

impl ProfileState {
    pub fn new() -> Self {
        Self {
            mode: ProfileMode::Switch,
            cursor: 0,
            scroll: 0,
            input: String::new(),
            copy_current: true,
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self, max: usize) {
        if self.cursor + 1 < max {
            self.cursor += 1;
        }
    }

    pub fn scroll_for_visible(&self, visible: usize, total: usize) -> (usize, usize) {
        if total == 0 {
            return (0, 0);
        }
        let scroll = if self.cursor >= visible {
            self.cursor - visible + 1
        } else {
            0
        };
        let end = (scroll + visible).min(total);
        (scroll, end)
    }

    pub fn cycle_mode(&mut self) {
        self.mode = self.mode.next();
        self.cursor = 0;
        self.scroll = 0;
    }
}
