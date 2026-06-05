/// Searchable keybind reference dialog.
pub struct HelpState {
    pub cursor: usize,
    pub scroll: usize,
}

impl HelpState {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            scroll: 0,
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
}

/// Static keybind reference data.
pub struct KeybindEntry {
    pub key: &'static str,
    pub description: &'static str,
}

pub const KEYBINDS: &[KeybindEntry] = &[
    // Navigation
    KeybindEntry {
        key: "j / k",
        description: "Navigate up / down",
    },
    KeybindEntry {
        key: "h / l",
        description: "Navigate miller columns (files panel)",
    },
    KeybindEntry {
        key: "Tab",
        description: "Switch focus between panels",
    },
    KeybindEntry {
        key: "/",
        description: "Fuzzy search apps or files",
    },
    // Apps panel
    KeybindEntry {
        key: "o",
        description: "Open primary config for selected app (apps panel)",
    },
    KeybindEntry {
        key: "x",
        description: "Remove app from roost",
    },
    KeybindEntry {
        key: "f",
        description: "Import app from another profile",
    },
    KeybindEntry {
        key: "m",
        description: "Copy app to another profile",
    },
    // Files panel
    KeybindEntry {
        key: "e / Enter",
        description: "Edit file in $EDITOR (files panel)",
    },
    KeybindEntry {
        key: "p",
        description: "Set file as primary config (files panel)",
    },
    // Management
    KeybindEntry {
        key: "a",
        description: "Add new app to roost",
    },
    KeybindEntry {
        key: "i",
        description: "Manage ignore patterns",
    },
    KeybindEntry {
        key: "P",
        description: "Switch / create / delete profiles",
    },
    // Git
    KeybindEntry {
        key: "s",
        description: "Save changes (git commit)",
    },
    KeybindEntry {
        key: "S",
        description: "Sync with remote (pull + push)",
    },
    KeybindEntry {
        key: "g",
        description: "View git log",
    },
    KeybindEntry {
        key: "d",
        description: "Show git diff",
    },
    KeybindEntry {
        key: "u",
        description: "Undo last commit",
    },
    KeybindEntry {
        key: "r",
        description: "Rollback to selected commit (git log dialog only)",
    },
    // System
    KeybindEntry {
        key: "?",
        description: "Show this help dialog",
    },
    KeybindEntry {
        key: "q / Esc",
        description: "Quit / close dialog",
    },
];
