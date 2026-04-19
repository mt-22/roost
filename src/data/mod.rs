use std::collections::HashSet;

const KNOWN_APPS: &str = include_str!("known_apps.txt");
const KNOWN_DOTFILES: &str = include_str!("known_dotfiles.txt");

fn parse_list(raw: &'static str) -> HashSet<&'static str> {
    raw.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| line.trim())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn known_apps() -> HashSet<&'static str> {
    parse_list(KNOWN_APPS)
}

pub fn known_dotfiles() -> HashSet<&'static str> {
    parse_list(KNOWN_DOTFILES)
}

pub const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "node_modules",
    ".git",
    ".DS_Store",
    "*.log",
    "*.tmp",
    "*.bak",
    "*.swp",
    "Thumbs.db",
    "__pycache__",
    ".cache",
    ".npm",
    ".venv",
    "*.pyc",
    ".tox",
    "dist",
    "build",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_apps_not_empty() {
        let apps = known_apps();
        assert!(!apps.is_empty());
        assert!(apps.contains("nvim"));
        assert!(apps.contains("git"));
    }

    #[test]
    fn known_dotfiles_not_empty() {
        let files = known_dotfiles();
        assert!(!files.is_empty());
        assert!(files.contains(".zshrc"));
        assert!(files.contains(".gitconfig"));
    }

    #[test]
    fn comments_and_blanks_filtered() {
        let apps = known_apps();
        assert!(!apps.contains(""));
        assert!(!apps.contains("# Shells"));
    }

    #[test]
    fn default_ignore_patterns_has_16() {
        assert_eq!(DEFAULT_IGNORE_PATTERNS.len(), 16);
    }
}
