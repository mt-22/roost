pub struct UndoConfirmState {
    pub message: String,
    pub date: String,
}

impl UndoConfirmState {
    pub fn new(entry: &crate::git::LogEntry) -> Self {
        Self {
            message: entry.message.clone(),
            date: entry.date.clone(),
        }
    }
}
