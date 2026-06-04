/// Inline diff viewer dialog.
pub struct DiffViewState {
    pub lines: Vec<String>,
    pub scroll: usize,
}

impl DiffViewState {
    pub fn new(diff_text: &str) -> Self {
        let lines: Vec<String> = diff_text.lines().map(|s| s.to_string()).collect();
        Self { lines, scroll: 0 }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, visible: usize) {
        let max = self.lines.len().saturating_sub(visible);
        if self.scroll < max {
            self.scroll += 1;
        }
    }
}
