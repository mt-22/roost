pub struct DiffViewState {
    pub diff: String,
    pub scroll: usize,
    pub title: String,
    pub confirm_sync: bool,
}

impl DiffViewState {
    pub fn new(title: String, diff: String, confirm_sync: bool) -> Self {
        Self {
            diff,
            scroll: 0,
            title,
            confirm_sync,
        }
    }

    pub fn lines(&self) -> Vec<&str> {
        self.diff.lines().collect()
    }

    pub fn scroll_down(&mut self) {
        let total = self.lines().len();
        if self.scroll + 1 < total {
            self.scroll += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }
}
