/// A single keybind entry shown in the help dialog.
pub struct Keybind {
    pub key: &'static str,
    /// Short label for what it does.
    pub action: &'static str,
    /// Longer description shown next to the action.
    pub description: &'static str,
    /// Optional context tag shown on the right (e.g. "[apps]", "[files]").
    pub context: Option<&'static str>,
}

/// Full list of keybinds, in display order.
pub const KEYBINDS: &[Keybind] = &[
    Keybind {
        key: "j / k",
        action: "Navigate",
        description: "Move cursor up / down",
        context: None,
    },
    Keybind {
        key: "Tab",
        action: "Switch focus",
        description: "Toggle between Apps and Files",
        context: None,
    },
    Keybind {
        key: "h / l",
        action: "Columns",
        description: "Navigate miller columns",
        context: Some("[files]"),
    },
    Keybind {
        key: "e  Enter",
        action: "Edit",
        description: "Open file in $EDITOR",
        context: Some("[files]"),
    },
    Keybind {
        key: "o",
        action: "Open primary",
        description: "Open primary config in $EDITOR",
        context: Some("[apps]"),
    },
    Keybind {
        key: "p",
        action: "Set primary",
        description: "Mark file as primary config",
        context: Some("[files]"),
    },
    Keybind {
        key: "/",
        action: "Search",
        description: "Fuzzy search",
        context: None,
    },
    Keybind {
        key: "a",
        action: "Add app",
        description: "Ingest a new app into roost",
        context: None,
    },
    Keybind {
        key: "x",
        action: "Remove app",
        description: "Stop managing app, restore files",
        context: Some("[apps]"),
    },
    Keybind {
        key: "f",
        action: "Import symlink",
        description: "Import an app from another profile via symlink",
        context: Some("[apps]"),
    },
    Keybind {
        key: "m",
        action: "Paste into",
        description: "Copy app's files to another profile",
        context: Some("[apps]"),
    },
    Keybind {
        key: "i",
        action: "Ignore patterns",
        description: "Add or remove ignore patterns",
        context: None,
    },
    Keybind {
        key: "P",
        action: "Profiles",
        description: "Switch, create, or delete profiles",
        context: None,
    },
    Keybind {
        key: "s",
        action: "Sync",
        description: "Commit and sync with remote",
        context: None,
    },
    Keybind {
        key: "g",
        action: "Git log",
        description: "Browse recent commits",
        context: None,
    },
    Keybind {
        key: "d",
        action: "Diff",
        description: "Show uncommitted changes",
        context: None,
    },
    Keybind {
        key: "u",
        action: "Undo",
        description: "Undo last commit (destructive)",
        context: None,
    },
    Keybind {
        key: "?",
        action: "Help",
        description: "Show this keybind reference",
        context: None,
    },
    Keybind {
        key: "q  Esc",
        action: "Quit",
        description: "Exit roost",
        context: None,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpFocus {
    Search,
    List,
}

pub struct HelpDialogState {
    pub query: String,
    pub scroll: usize,
    pub focus: HelpFocus,
}

impl HelpDialogState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            scroll: 0,
            focus: HelpFocus::Search,
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            HelpFocus::Search => HelpFocus::List,
            HelpFocus::List => HelpFocus::Search,
        };
    }

    pub fn push(&mut self, ch: char) {
        self.query.push(ch);
        self.scroll = 0;
    }

    pub fn pop(&mut self) -> bool {
        let removed = self.query.pop().is_some();
        self.scroll = 0;
        removed
    }

    /// Return indices into KEYBINDS that match the current query.
    pub fn matches(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..KEYBINDS.len()).collect();
        }
        let q = self.query.to_lowercase();
        KEYBINDS
            .iter()
            .enumerate()
            .filter(|(_, kb)| {
                kb.key.to_lowercase().contains(&q)
                    || kb.action.to_lowercase().contains(&q)
                    || kb.description.to_lowercase().contains(&q)
                    || kb
                        .context
                        .map(|c| c.to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn scroll_down(&mut self, max: usize) {
        if max > 0 {
            self.scroll = (self.scroll + 1).min(max - 1);
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }
}
