use crate::app::Application;
use crate::tui::state::MillerEntry;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

static KNOWN_APPS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("../data/known_apps.txt")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
});

static KNOWN_DOTFILES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("../data/known_dotfiles.txt")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
});

#[derive(Debug, Clone)]
pub struct SourceEntry {
    pub path: PathBuf,
    pub name: String,
}

impl MillerEntry for SourceEntry {
    fn path(&self) -> &Path {
        &self.path
    }

    fn is_dir(&self) -> bool {
        self.path.is_dir()
    }
}

/// Returns the hardcoded list of likely source directories, filtered to those that exist.
pub fn get_likely_sources() -> Vec<PathBuf> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let candidates = [
        Some(home.join(".config")),
        dirs::config_dir(), // ~/Library/Application Support on macOS
        Some(home.join(".local/bin")),
        Some(home.join(".ssh")),
        Some(home.clone()),
    ];

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .flatten()
        .filter(|p| p.exists() && seen.insert(p.clone()))
        .collect()
}

/// Human-readable tab label for a source path.
pub fn source_label(source: &Path) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    if source == home {
        "$HOME".to_string()
    } else if let Ok(rel) = source.strip_prefix(&home) {
        format!("~/{}", rel.display())
    } else {
        source.display().to_string()
    }
}

/// Scan a source directory and return its entries sorted by app-likelihood.
///
/// When `dotfiles_only` is true, only entries starting with '.' are included
/// (used for the $HOME source tab). The Browse tab passes false to show everything.
pub fn scan_source(
    source: &Path,
    ignored: &HashSet<String>,
    dotfiles_only: bool,
) -> color_eyre::Result<Vec<SourceEntry>> {
    let entries = fs::read_dir(source)?;
    let mut result: Vec<SourceEntry> = Vec::new();

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if is_ignored(&name, ignored) {
            continue;
        }

        if dotfiles_only && !name.starts_with('.') {
            continue;
        }

        let path = entry.path();
        result.push(SourceEntry { path, name });
    }

    result.sort_by(|a, b| {
        app_confidence(b)
            .cmp(&app_confidence(a))
            .then(a.name.cmp(&b.name))
    });

    Ok(result)
}

/// Score an entry by how likely it is to be a managed application config.
/// Higher = more likely.
fn app_confidence(entry: &SourceEntry) -> u16 {
    let name_lower = entry.name.to_lowercase();
    let bare_name = name_lower.strip_prefix('.').unwrap_or(&name_lower);

    // Well-known dotfile in $HOME (e.g. .zshrc, .gitconfig)
    if KNOWN_DOTFILES.contains(name_lower.as_str()) {
        return 200;
    }

    if entry.path.is_dir() {
        // Well-known app directory (e.g. nvim, hyprland, eww)
        if KNOWN_APPS.contains(bare_name) {
            return 150;
        }
        // Directory with config-like children
        if has_config_children(&entry.path) {
            return 100;
        }
        // Other directory
        return 50;
    }

    // File that looks like a config
    if looks_like_config_file(&entry.name) {
        return 80;
    }

    // Unknown file
    10
}

/// Check if a directory contains files that look like config files.
fn has_config_children(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    entries.filter_map(|e| e.ok()).any(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        looks_like_config_file(&name)
    })
}

fn looks_like_config_file(name: &str) -> bool {
    const CONFIG_EXTENSIONS: &[&str] = &[
        ".toml", ".yml", ".yaml", ".json", ".conf", ".ini", ".cfg", ".lua", ".vim",
    ];
    const CONFIG_NAMES: &[&str] = &["config", "settings", "init.lua", "init.vim"];

    if CONFIG_NAMES.contains(&name) {
        return true;
    }
    if name.ends_with("rc") && !name.contains('.') {
        return true;
    }
    CONFIG_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
}

/// Check if a name should be ignored.
///
/// Supports two pattern forms only:
/// - Exact match (e.g. `"node_modules"`)
/// - Suffix wildcard (e.g. `"*.log"`)
///
/// Patterns like `prefix*`, `dir/name`, or `**/glob` are NOT supported
/// and will silently fail to match.
pub fn is_ignored(name: &str, ignored: &HashSet<String>) -> bool {
    if ignored.contains(name) {
        return true;
    }
    ignored
        .iter()
        .any(|pat| pat.starts_with("*.") && name.ends_with(&pat[1..]))
}

/// Recursively collect all files in a directory, respecting ignores.
pub fn collect_files_recursive(
    dir: &Path,
    ignored: &HashSet<String>,
) -> color_eyre::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(files),
    };

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if is_ignored(&name, ignored) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files_recursive(&path, ignored)?);
        } else {
            files.push(path);
        }
    }

    Ok(files)
}

/// Convert a selected SourceEntry into an Application.
pub fn entry_to_application(
    entry: &SourceEntry,
    ignored: &HashSet<String>,
    profile: &str,
) -> color_eyre::Result<Application> {
    // Scan files only to guess the primary config; they are no longer stored.
    let files = if entry.path.is_dir() {
        collect_files_recursive(&entry.path, ignored)?
    } else {
        vec![entry.path.clone()]
    };

    let primary_config = guess_primary_config(&entry.name, &files)
        .or_else(|| files.first().filter(|_| files.len() == 1).cloned());

    Ok(Application {
        name: entry.name.clone(),
        primary_config,
        on_profiles: vec![profile.to_string()],
    })
}

/// Heuristic: find the most likely "primary" config file in a set of files.
fn guess_primary_config(app_name: &str, files: &[PathBuf]) -> Option<PathBuf> {
    let candidates = [
        format!("{}.toml", app_name),
        format!("{}.yml", app_name),
        format!("{}.yaml", app_name),
        format!("{}.json", app_name),
        format!("{}.conf", app_name),
        "config.toml".to_string(),
        "config.yml".to_string(),
        "config.yaml".to_string(),
        "config.json".to_string(),
        "config".to_string(),
        "init.vim".to_string(),
        "init.lua".to_string(),
    ];

    for candidate in &candidates {
        if let Some(f) = files.iter().find(|f| {
            f.file_name()
                .map(|n| n.to_string_lossy().eq_ignore_ascii_case(candidate))
                .unwrap_or(false)
        }) {
            return Some(f.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests;
