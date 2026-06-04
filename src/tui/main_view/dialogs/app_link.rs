/// App link wizard: import-from (`f`) or paste-into (`m`).
///
/// Two-step flow:
/// 1. Select target profile from list.
/// 2. For import: select app from that profile.
///    For paste: confirm copy of current app to that profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLinkAction {
    Import,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLinkStep {
    PickProfile,
    PickApp,
    ConfirmCopy,
}

pub struct AppLinkState {
    pub action: AppLinkAction,
    pub step: AppLinkStep,
    pub cursor: usize,
    pub scroll: usize,
    pub selected_profile: Option<String>,
}

impl AppLinkState {
    pub fn new(action: AppLinkAction) -> Self {
        Self {
            action,
            step: AppLinkStep::PickProfile,
            cursor: 0,
            scroll: 0,
            selected_profile: None,
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

    pub fn advance_step(&mut self) {
        self.step = match self.step {
            AppLinkStep::PickProfile => {
                if self.action == AppLinkAction::Copy {
                    AppLinkStep::ConfirmCopy
                } else {
                    AppLinkStep::PickApp
                }
            }
            AppLinkStep::PickApp => AppLinkStep::PickProfile,
            AppLinkStep::ConfirmCopy => AppLinkStep::PickProfile,
        };
        self.cursor = 0;
        self.scroll = 0;
    }
}
