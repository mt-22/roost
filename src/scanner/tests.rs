use super::*;
use std::fs;
use tempfile::TempDir;

fn create_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, "").unwrap();
    path
}

fn create_dir(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn score_known_dotfile() {
    let score = score_item(".zshrc", Path::new("/fake"), &ItemType::File);
    assert_eq!(score, 200);
}

#[test]
fn score_known_app_dir() {
    let score = score_item("nvim", Path::new("/fake"), &ItemType::Dir);
    assert_eq!(score, 150);
}

#[test]
fn score_dir_with_config_children() {
    let tmp = TempDir::new().unwrap();
    let dir = create_dir(tmp.path(), "myapp");
    create_file(&dir, "config.toml");
    let score = score_item("myapp", &dir, &ItemType::Dir);
    assert_eq!(score, 100);
}

#[test]
fn score_dir_without_config_children() {
    let tmp = TempDir::new().unwrap();
    let dir = create_dir(tmp.path(), "myapp");
    create_file(&dir, "readme.txt");
    let score = score_item("myapp", &dir, &ItemType::Dir);
    assert_eq!(score, 50);
}

#[test]
fn score_config_file() {
    let score = score_item("settings.toml", Path::new("/fake"), &ItemType::File);
    assert_eq!(score, 80);
}

#[test]
fn score_unknown_file() {
    let score = score_item("readme.txt", Path::new("/fake"), &ItemType::File);
    assert_eq!(score, 10);
}

#[test]
fn ignore_exact_match() {
    let ignored: HashSet<String> = ["node_modules".into()].into();
    assert!(matches_ignore("node_modules", &ignored));
}

#[test]
fn ignore_suffix_wildcard() {
    let ignored: HashSet<String> = ["*.log".into()].into();
    assert!(matches_ignore("error.log", &ignored));
    assert!(!matches_ignore("error.txt", &ignored));
}

#[test]
fn no_ignore_when_not_matching() {
    let ignored: HashSet<String> = ["*.log".into(), "node_modules".into()].into();
    assert!(!matches_ignore("config.toml", &ignored));
}

#[test]
fn scan_directory_discovers_items() {
    let tmp = TempDir::new().unwrap();
    create_dir(tmp.path(), "nvim");
    create_file(tmp.path(), ".zshrc");

    let ignored = HashSet::new();
    let results = scan_directory(tmp.path(), &ignored);

    assert!(!results.is_empty());
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"nvim"));
    assert!(names.contains(&".zshrc"));
}

#[test]
fn scan_directory_respects_ignores() {
    let tmp = TempDir::new().unwrap();
    create_dir(tmp.path(), "node_modules");
    create_dir(tmp.path(), "nvim");

    let ignored: HashSet<String> = ["node_modules".into()].into();
    let results = scan_directory(tmp.path(), &ignored);

    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(!names.contains(&"node_modules"));
    assert!(names.contains(&"nvim"));
}

#[test]
fn scan_directory_skips_known_non_configs() {
    let tmp = TempDir::new().unwrap();
    create_dir(tmp.path(), "Documents");
    create_dir(tmp.path(), "Pictures");
    create_dir(tmp.path(), "caches");
    create_dir(tmp.path(), "nvim");

    let ignored = HashSet::new();
    let results = scan_directory(tmp.path(), &ignored);

    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(!names.contains(&"Documents"));
    assert!(!names.contains(&"Pictures"));
    assert!(!names.contains(&"caches"));
    assert!(names.contains(&"nvim"));
}

#[test]
fn scan_directory_sorted_by_confidence() {
    let tmp = TempDir::new().unwrap();
    create_file(tmp.path(), ".zshrc");
    create_file(tmp.path(), "random.txt");
    create_dir(tmp.path(), ".config");

    let ignored = HashSet::new();
    let results = scan_directory(tmp.path(), &ignored);

    for window in results.windows(2) {
        assert!(window[0].confidence >= window[1].confidence);
    }
}

#[test]
fn scan_sources_combines_multiple_directories() {
    let tmp = TempDir::new().unwrap();
    let dir_a = create_dir(tmp.path(), "a");
    let dir_b = create_dir(tmp.path(), "b");

    create_file(&dir_a, "nvim");
    create_file(&dir_b, "vim");

    let sources = vec![
        ScanSource::new(dir_a.clone(), 0),
        ScanSource::new(dir_b.clone(), 0),
    ];
    let ignored = HashSet::new();
    let results = scan_sources(&sources, &ignored);

    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"nvim"));
    assert!(names.contains(&"vim"));
}

#[test]
fn scan_sources_deduplicates_by_path() {
    let tmp = TempDir::new().unwrap();
    let dir_a = create_dir(tmp.path(), "a");

    create_file(&dir_a, "nvim");

    let sources = vec![
        ScanSource::new(dir_a.clone(), 0),
        ScanSource::new(dir_a.clone(), 0),
    ];
    let ignored = HashSet::new();
    let results = scan_sources(&sources, &ignored);

    let count = results.iter().filter(|r| r.name == "nvim").count();
    assert_eq!(count, 1);
}

#[test]
fn scan_sources_applies_positive_modifier() {
    let tmp = TempDir::new().unwrap();
    let dir = create_dir(tmp.path(), "src");

    create_file(&dir, "settings.toml");

    let sources = vec![ScanSource::new(dir.clone(), 50)];
    let ignored = HashSet::new();
    let results = scan_sources(&sources, &ignored);

    let item = results.iter().find(|r| r.name == "settings.toml").unwrap();
    assert_eq!(item.confidence, 130); // 80 + 50
}

#[test]
fn scan_sources_applies_negative_modifier() {
    let tmp = TempDir::new().unwrap();
    let dir = create_dir(tmp.path(), "src");

    create_file(&dir, ".zshrc");

    let sources = vec![ScanSource::new(dir.clone(), -50)];
    let ignored = HashSet::new();
    let results = scan_sources(&sources, &ignored);

    let item = results.iter().find(|r| r.name == ".zshrc").unwrap();
    assert_eq!(item.confidence, 150); // 200 - 50
}

#[test]
fn scan_sources_discards_below_threshold() {
    let tmp = TempDir::new().unwrap();
    let dir = create_dir(tmp.path(), "src");

    create_file(&dir, "readme.txt"); // scores 10, below 80 threshold

    let sources = vec![ScanSource::new(dir.clone(), 0)];
    let ignored = HashSet::new();
    let results = scan_sources(&sources, &ignored);

    assert!(!results.iter().any(|r| r.name == "readme.txt"));
}

#[test]
fn scan_sources_keeps_at_threshold() {
    let tmp = TempDir::new().unwrap();
    let dir = create_dir(tmp.path(), "src");

    create_file(&dir, "settings.toml"); // scores 80, at threshold

    let sources = vec![ScanSource::new(dir.clone(), 0)];
    let ignored = HashSet::new();
    let results = scan_sources(&sources, &ignored);

    assert!(results.iter().any(|r| r.name == "settings.toml"));
    assert_eq!(
        results
            .iter()
            .find(|r| r.name == "settings.toml")
            .unwrap()
            .confidence,
        80
    );
}

#[test]
fn scan_sources_penalized_items_above_threshold_are_kept() {
    let tmp = TempDir::new().unwrap();
    let dir = create_dir(tmp.path(), "src");

    // nvim is a known app (scores 150), with -50 modifier = 100, above threshold
    create_dir(&dir, "nvim");

    let sources = vec![ScanSource::new(dir.clone(), -50)];
    let ignored = HashSet::new();
    let results = scan_sources(&sources, &ignored);

    assert!(results.iter().any(|r| r.name == "nvim"));
    assert_eq!(
        results
            .iter()
            .find(|r| r.name == "nvim")
            .unwrap()
            .confidence,
        100
    );
}

#[test]
fn default_scan_sources_includes_home_and_config() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();

    let sources = default_scan_sources(home);
    let paths: Vec<PathBuf> = sources.iter().map(|s| s.path.clone()).collect();

    assert!(paths.contains(&home.join(".config")));
    assert!(paths.contains(&home.to_path_buf()));
}

#[test]
fn default_scan_sources_penalizes_secondary() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();

    let sources = default_scan_sources(home);
    let local_bin = sources.iter().find(|s| s.path == home.join(".local/bin"));
    let ssh = sources.iter().find(|s| s.path == home.join(".ssh"));

    assert!(local_bin.is_some());
    assert_eq!(local_bin.unwrap().modifier, -50);
    assert!(ssh.is_some());
    assert_eq!(ssh.unwrap().modifier, -50);
}
