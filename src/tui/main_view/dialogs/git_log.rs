pub struct GitLogDialogState {
    pub entries: Vec<crate::git::LogEntry>,
    pub cursor: usize,
    pub confirm_rollback: Option<String>,
}

impl GitLogDialogState {
    pub fn new(entries: Vec<crate::git::LogEntry>) -> Self {
        Self {
            entries,
            cursor: 0,
            confirm_rollback: None,
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = (self.cursor + 1).min(self.entries.len() - 1);
        }
    }

    pub fn selected_hash(&self) -> Option<String> {
        self.entries.get(self.cursor).map(|e| e.hash.clone())
    }

    pub fn start_rollback(&mut self) {
        if let Some(hash) = self.selected_hash() {
            self.confirm_rollback = Some(hash);
        }
    }

    pub fn cancel_rollback(&mut self) {
        self.confirm_rollback = None;
    }
}
