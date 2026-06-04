/// Undo confirmation dialog.
pub struct UndoState {
    pub message: String,
}

impl UndoState {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
