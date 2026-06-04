/// Git log browser dialog.
pub struct GitLogState {
    pub commits: Vec<crate::git::CommitInfo>,
    pub cursor: usize,
    pub scroll: usize,
}

impl GitLogState {
    pub fn new(commits: Vec<crate::git::CommitInfo>) -> Self {
        Self {
            commits,
            cursor: 0,
            scroll: 0,
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.commits.len() {
            self.cursor += 1;
        }
    }

    pub fn scroll_for_visible(&self, visible: usize) -> (usize, usize) {
        let total = self.commits.len();
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

    pub fn selected_hash(&self) -> Option<&str> {
        self.commits.get(self.cursor).map(|c| c.hash.as_str())
    }
}
