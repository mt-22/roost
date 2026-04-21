use color_eyre::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    Dir,
    File,
}

#[derive(Debug, Clone)]
pub struct DiscoveredItem {
    pub path: PathBuf,
    pub name: String,
    pub confidence: u32,
    pub item_type: ItemType,
}

const CONFIG_EXTENSIONS: &[&str] = &[
    "toml", "yaml", "yml", "json", "conf", "ini", "cfg", "rc", "xml",
];

fn score_item(name: &str, path: &Path, item_type: &ItemType) -> u32 {
    let known_apps = crate::data::known_apps();
    let known_dotfiles = crate::data::known_dotfiles();

    let lower = name.to_lowercase();

    if known_dotfiles.contains(name) {
        return 200;
    }

    if known_apps.contains(lower.as_str()) {
        return 150;
    }

    match item_type {
        ItemType::Dir => {
            if has_config_children(path) {
                100
            } else {
                50
            }
        }
        ItemType::File => {
            if is_config_file(name) {
                80
            } else {
                10
            }
        }
    }
}

fn has_config_children(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if is_config_file(&name_str) || name_str.starts_with('.') {
            return true;
        }
    }
    false
}

fn is_config_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    CONFIG_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{}", ext)))
}

fn matches_ignore(name: &str, ignored: &HashSet<String>) -> bool {
    if ignored.contains(name) {
        return true;
    }
    for pattern in ignored {
        if let Some(suffix) = pattern.strip_prefix('*') {
            if name.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

fn scan_directory(
    dir: &Path,
    max_depth: u32,
    ignored: &HashSet<String>,
    results: &mut Vec<DiscoveredItem>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') && name == ".DS_Store" {
            continue;
        }
        if matches_ignore(&name, ignored) {
            continue;
        }

        let item_type = if path.is_dir() {
            ItemType::Dir
        } else {
            ItemType::File
        };

        let confidence = score_item(&name, &path, &item_type);
        results.push(DiscoveredItem {
            path: path.clone(),
            name,
            confidence,
            item_type,
        });

        if item_type == ItemType::Dir && max_depth > 0 {
            scan_directory(&path, max_depth - 1, ignored, results);
        }
    }
}

pub fn scan_home(home: &Path, ignored: &HashSet<String>) -> Result<Vec<DiscoveredItem>> {
    let mut results = Vec::new();

    let scan_targets = [
        ("config", home.join(".config")),
        ("library", home.join("Library/Application Support")),
        ("local_bin", home.join(".local/bin")),
        ("ssh", home.join(".ssh")),
    ];

    for (_, dir) in &scan_targets {
        if dir.exists() {
            scan_directory(dir, 0, ignored, &mut results);
        }
    }

    // Scan $HOME direct children (depth 0 only)
    scan_directory(home, 0, ignored, &mut results);

    results.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    Ok(results)
}

#[cfg(test)]
mod tests;
