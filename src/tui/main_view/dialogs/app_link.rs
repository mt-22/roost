#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppLinkMode {
    LinkFrom { step: LinkFromStep },
    PasteInto { app_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkFromStep {
    PickProfile,
    PickApp { source_profile: String },
}

pub struct AppLinkDialogState {
    pub mode: AppLinkMode,
    pub cursor: usize,
}

impl AppLinkDialogState {
    pub fn link_from() -> Self {
        Self {
            mode: AppLinkMode::LinkFrom {
                step: LinkFromStep::PickProfile,
            },
            cursor: 0,
        }
    }

    pub fn paste_into(app_name: String) -> Self {
        Self {
            mode: AppLinkMode::PasteInto { app_name },
            cursor: 0,
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self, count: usize) {
        if count > 0 {
            self.cursor = (self.cursor + 1).min(count - 1);
        }
    }

    pub fn accept<'a>(&self, items: &'a [String]) -> Option<&'a String> {
        items.get(self.cursor)
    }
}
