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

fn is_known_non_config(name: &str) -> bool {
    let known_non_configs = crate::data::known_non_configs();
    let lower = name.to_lowercase();
    known_non_configs.contains(lower.as_str())
}

fn matches_ignore(name: &str, ignored: &HashSet<String>) -> bool {
    if ignored.contains(name) {
        return true;
    }
    for pattern in ignored {
        if let Some(suffix) = pattern.strip_prefix('*')
            && name.ends_with(suffix)
        {
            return true;
        }
    }
    false
}

pub fn scan_directory(dir: &Path, ignored: &HashSet<String>) -> Vec<DiscoveredItem> {
    let mut results = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return results;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name == ".DS_Store" {
            continue;
        }
        if is_known_non_config(&name) {
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
            path,
            name,
            confidence,
            item_type,
        });
    }
    results.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    results
}

/// A configurable source directory for scanning.
#[derive(Debug, Clone)]
pub struct ScanSource {
    pub path: PathBuf,
    /// Modifier applied to the base confidence score from this source.
    /// Negative values penalize configs found in secondary locations.
    pub modifier: i32,
}

impl ScanSource {
    pub fn new(path: PathBuf, modifier: i32) -> Self {
        Self { path, modifier }
    }
}

/// Scan multiple source directories, apply source modifiers, filter out
/// low-confidence items (< 80), and deduplicate by path.
///
/// Items with final score >= 150 are considered for auto-selection during init.
/// Items with final score < 80 are discarded entirely.
pub fn scan_sources(sources: &[ScanSource], ignored: &HashSet<String>) -> Vec<DiscoveredItem> {
    let mut seen_paths = HashSet::new();
    let mut all_items = Vec::new();

    for source in sources {
        if !source.path.exists() {
            continue;
        }
        let items = scan_directory(&source.path, ignored);
        for mut item in items {
            if !seen_paths.insert(item.path.clone()) {
                continue;
            }

            // Apply source modifier
            let adjusted = if source.modifier >= 0 {
                item.confidence.saturating_add(source.modifier as u32)
            } else {
                item.confidence.saturating_sub((-source.modifier) as u32)
            };
            item.confidence = adjusted;

            // Discard items below threshold
            if item.confidence >= 80 {
                all_items.push(item);
            }
        }
    }

    all_items.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    all_items
}

/// Build the default list of scan sources for the current platform.
pub fn default_scan_sources(home: &Path) -> Vec<ScanSource> {
    let mut sources = vec![
        ScanSource::new(home.join(".config"), 0),
        ScanSource::new(home.to_path_buf(), 0),
    ];

    // Secondary sources (penalized)
    sources.push(ScanSource::new(
        home.join("Library/Application Support"),
        -50,
    ));
    sources.push(ScanSource::new(home.join(".local/bin"), -50));
    sources.push(ScanSource::new(home.join(".ssh"), -50));

    sources
}

#[cfg(test)]
mod tests;
