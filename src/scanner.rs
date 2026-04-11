use crate::app::Application;
use crate::tui::state::MillerEntry;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

static KNOWN_APPS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("data/known_apps.txt")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
});

static KNOWN_DOTFILES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("data/known_dotfiles.txt")
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
mod tests {
    use super::*;
    use std::fs;

    fn make_entry(name: &str, path: &Path) -> SourceEntry {
        SourceEntry {
            path: path.to_path_buf(),
            name: name.to_string(),
        }
    }

    fn make_file_entry(name: &str, dir: &Path) -> SourceEntry {
        let path = dir.join(name);
        fs::write(&path, "").unwrap();
        make_entry(name, &path)
    }

    fn make_dir_entry(name: &str, parent: &Path) -> SourceEntry {
        let path = parent.join(name);
        fs::create_dir_all(&path).unwrap();
        make_entry(name, &path)
    }

    // ── is_ignored ──────────────────────────────────────────────────────────

    #[test]
    fn is_ignored_exact_match() {
        let mut ignored = HashSet::new();
        ignored.insert("node_modules".to_string());
        assert!(is_ignored("node_modules", &ignored));
    }

    #[test]
    fn is_ignored_suffix_wildcard() {
        let mut ignored = HashSet::new();
        ignored.insert("*.log".to_string());
        assert!(is_ignored("debug.log", &ignored));
    }

    #[test]
    fn is_ignored_no_match() {
        let mut ignored = HashSet::new();
        ignored.insert("*.log".to_string());
        assert!(!is_ignored("nvim", &ignored));
    }

    #[test]
    fn is_ignored_empty_set() {
        let ignored = HashSet::new();
        assert!(!is_ignored("anything", &ignored));
    }

    #[test]
    fn is_ignored_suffix_not_substring() {
        let mut ignored = HashSet::new();
        ignored.insert("*.log".to_string());
        assert!(!is_ignored("logfile", &ignored));
    }

    // ── looks_like_config_file ──────────────────────────────────────────────

    #[test]
    fn config_file_known_extensions() {
        for ext in &[
            ".toml", ".yml", ".yaml", ".json", ".conf", ".ini", ".cfg", ".lua", ".vim",
        ] {
            assert!(
                looks_like_config_file(&format!("foo{}", ext)),
                "expected {} to be a config extension",
                ext
            );
        }
    }

    #[test]
    fn config_file_known_names() {
        assert!(looks_like_config_file("config"));
        assert!(looks_like_config_file("settings"));
        assert!(looks_like_config_file("init.lua"));
        assert!(looks_like_config_file("init.vim"));
    }

    #[test]
    fn config_file_rc_files() {
        assert!(looks_like_config_file("zshrc"));
        assert!(looks_like_config_file("bashrc"));
    }

    #[test]
    fn config_file_negative() {
        assert!(!looks_like_config_file("README.md"));
        assert!(!looks_like_config_file("screenshot.png"));
    }

    // ── app_confidence ──────────────────────────────────────────────────────

    #[test]
    fn confidence_known_dotfile() {
        let tmp = tempfile::tempdir().unwrap();
        let entry = make_file_entry(".zshrc", tmp.path());
        assert_eq!(app_confidence(&entry), 200);
    }

    #[test]
    fn confidence_known_app_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let entry = make_dir_entry("nvim", tmp.path());
        assert_eq!(app_confidence(&entry), 150);
    }

    #[test]
    fn confidence_dir_with_config_children() {
        let tmp = tempfile::tempdir().unwrap();
        let dir_path = tmp.path().join("myapp");
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join("config.toml"), "").unwrap();
        let entry = make_entry("myapp", &dir_path);
        assert_eq!(app_confidence(&entry), 100);
    }

    #[test]
    fn confidence_plain_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let entry = make_dir_entry("random_dir", tmp.path());
        assert_eq!(app_confidence(&entry), 50);
    }

    #[test]
    fn confidence_config_file_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let entry = make_file_entry("something.toml", tmp.path());
        assert_eq!(app_confidence(&entry), 80);
    }

    #[test]
    fn confidence_unknown_file() {
        let tmp = tempfile::tempdir().unwrap();
        let entry = make_file_entry("random.bin", tmp.path());
        assert_eq!(app_confidence(&entry), 10);
    }

    // ── scan_source ─────────────────────────────────────────────────────────

    #[test]
    fn scan_source_sorted_by_confidence() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("random.bin"), "").unwrap();
        fs::create_dir(tmp.path().join("nvim")).unwrap();
        fs::write(tmp.path().join(".zshrc"), "").unwrap();

        let ignored = HashSet::new();
        let entries = scan_source(tmp.path(), &ignored, false).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, [".zshrc", "nvim", "random.bin"]);
    }

    #[test]
    fn scan_source_respects_ignores() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();
        fs::write(tmp.path().join("debug.log"), "").unwrap();
        fs::write(tmp.path().join("keep.toml"), "").unwrap();

        let mut ignored = HashSet::new();
        ignored.insert("node_modules".to_string());
        ignored.insert("*.log".to_string());

        let entries = scan_source(tmp.path(), &ignored, false).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["keep.toml"]);
    }

    #[test]
    fn scan_source_dotfiles_only() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".hidden"), "").unwrap();
        fs::write(tmp.path().join("visible.toml"), "").unwrap();

        let ignored = HashSet::new();
        let entries = scan_source(tmp.path(), &ignored, true).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, [".hidden"]);
    }

    // ── collect_files_recursive ─────────────────────────────────────────────

    #[test]
    fn collect_files_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();
        fs::write(sub.join("b.txt"), "").unwrap();

        let ignored = HashSet::new();
        let files = collect_files_recursive(tmp.path(), &ignored).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"b.txt".to_string()));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn collect_files_respects_ignores() {
        let tmp = tempfile::tempdir().unwrap();
        let nm = tmp.path().join("node_modules");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("package.json"), "").unwrap();
        fs::write(tmp.path().join("keep.txt"), "").unwrap();

        let mut ignored = HashSet::new();
        ignored.insert("node_modules".to_string());
        let files = collect_files_recursive(tmp.path(), &ignored).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap().to_string_lossy(), "keep.txt");
    }

    // ── guess_primary_config ────────────────────────────────────────────────

    #[test]
    fn guess_primary_finds_appname_toml() {
        let files = vec![
            PathBuf::from("/some/path/nvim.toml"),
            PathBuf::from("/some/path/other.lua"),
        ];
        assert_eq!(
            guess_primary_config("nvim", &files),
            Some(PathBuf::from("/some/path/nvim.toml"))
        );
    }

    #[test]
    fn guess_primary_falls_back_to_config_toml() {
        let files = vec![
            PathBuf::from("/some/path/config.toml"),
            PathBuf::from("/some/path/other.lua"),
        ];
        assert_eq!(
            guess_primary_config("myapp", &files),
            Some(PathBuf::from("/some/path/config.toml"))
        );
    }

    #[test]
    fn guess_primary_returns_none() {
        let files = vec![
            PathBuf::from("/some/path/random.txt"),
            PathBuf::from("/some/path/other.bin"),
        ];
        assert_eq!(guess_primary_config("myapp", &files), None);
    }

    // ── entry_to_application ────────────────────────────────────────────────

    #[test]
    fn entry_to_application_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir_path = tmp.path().join("nvim");
        fs::create_dir_all(&dir_path).unwrap();

        let entry = make_entry("nvim", &dir_path);
        let ignored = HashSet::new();
        let app = entry_to_application(&entry, &ignored, "default").unwrap();
        assert_eq!(app.name, "nvim");
        assert_eq!(app.on_profiles, vec!["default"]);
    }

    #[test]
    fn entry_to_application_single_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("settings.toml");
        fs::write(&file_path, "key = value").unwrap();

        let entry = make_entry("settings.toml", &file_path);
        let ignored = HashSet::new();
        let app = entry_to_application(&entry, &ignored, "laptop").unwrap();
        assert_eq!(app.name, "settings.toml");
        assert!(app.primary_config.is_some());
        assert_eq!(app.on_profiles, vec!["laptop"]);
    }

    // ── source_label ────────────────────────────────────────────────────────

    #[test]
    fn source_label_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(source_label(&home), "$HOME");
    }

    #[test]
    fn source_label_home_subdir() {
        let home = dirs::home_dir().unwrap();
        let config = home.join(".config");
        assert_eq!(source_label(&config), "~/.config");
    }

    #[test]
    fn source_label_absolute_non_home() {
        assert_eq!(source_label(Path::new("/usr/local/bin")), "/usr/local/bin");
    }

    #[test]
    fn test_scan_source_empty_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let entries = scan_source(dir.path(), &HashSet::new(), false).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_scan_source_with_symlinked_entries() {
        let dir = tempfile::TempDir::new().unwrap();
        let real_dir = dir.path().join("real_nvim");
        fs::create_dir_all(&real_dir).unwrap();
        fs::write(real_dir.join("init.lua"), "").unwrap();

        let link = dir.path().join("nvim");
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        let entries = scan_source(dir.path(), &HashSet::new(), false).unwrap();
        assert!(entries.iter().any(|e| e.name == "nvim"));
    }

    #[test]
    fn test_is_ignored_star_dot_ext_matches_various() {
        let ignored: HashSet<String> = ["*.log".to_string()].into_iter().collect();
        assert!(is_ignored("app.log", &ignored));
        assert!(is_ignored("debug.log", &ignored));
        assert!(!is_ignored("log", &ignored));
        assert!(!is_ignored("log.txt", &ignored));
    }

    #[test]
    fn test_is_ignored_case_sensitive() {
        let ignored: HashSet<String> = ["Node_Modules".to_string()].into_iter().collect();
        assert!(is_ignored("Node_Modules", &ignored));
        assert!(!is_ignored("node_modules", &ignored));
    }

    #[test]
    fn test_collect_files_recursive_deep_nesting() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut current = dir.path().to_path_buf();
        for i in 0..5 {
            current = current.join(format!("level{}", i));
            fs::create_dir_all(&current).unwrap();
        }
        fs::write(current.join("deep.toml"), "deepest").unwrap();

        let files = collect_files_recursive(dir.path(), &HashSet::new()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("deep.toml"));
    }

    #[test]
    fn test_collect_files_recursive_empty_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        let files = collect_files_recursive(dir.path(), &HashSet::new()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_entry_to_application_sets_on_profiles() {
        let dir = tempfile::TempDir::new().unwrap();
        let app_dir = dir.path().join("nvim");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("init.lua"), "").unwrap();

        let entry = SourceEntry {
            path: app_dir,
            name: "nvim".to_string(),
        };
        let app = entry_to_application(&entry, &HashSet::new(), "my-profile").unwrap();
        assert_eq!(app.on_profiles, vec!["my-profile"]);
    }

    #[test]
    fn test_entry_to_application_file_primary_config_is_set() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join(".bashrc");
        fs::write(&file, "data").unwrap();

        let entry = SourceEntry {
            path: file,
            name: ".bashrc".to_string(),
        };
        let app = entry_to_application(&entry, &HashSet::new(), "default").unwrap();
        assert!(app.primary_config.is_some());
    }
}
