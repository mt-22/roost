/// Ignore pattern management dialog with two modes (Tab to cycle):
/// 1. Add — type a pattern to add to the ignore list
/// 2. Remove — select a pattern to remove from the ignore list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreMode {
    Add,
    Remove,
}

impl IgnoreMode {
    pub fn next(&self) -> Self {
        match self {
            IgnoreMode::Add => IgnoreMode::Remove,
            IgnoreMode::Remove => IgnoreMode::Add,
        }
    }
}

pub struct IgnoreState {
    pub mode: IgnoreMode,
    pub cursor: usize,
    pub scroll: usize,
    pub input: String,
}

impl IgnoreState {
    pub fn new() -> Self {
        Self {
            mode: IgnoreMode::Add,
            cursor: 0,
            scroll: 0,
            input: String::new(),
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
