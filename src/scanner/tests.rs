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
fn scan_home_discovers_items() {
    let tmp = TempDir::new().unwrap();
    let config_dir = create_dir(tmp.path(), ".config");
    create_dir(&config_dir, "nvim");
    create_file(tmp.path(), ".zshrc");

    let ignored = HashSet::new();
    let results = scan_home(tmp.path(), &ignored).unwrap();

    assert!(!results.is_empty());
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"nvim"));
    assert!(names.contains(&".zshrc"));
}

#[test]
fn scan_home_respects_ignores() {
    let tmp = TempDir::new().unwrap();
    let config_dir = create_dir(tmp.path(), ".config");
    create_dir(&config_dir, "node_modules");
    create_dir(&config_dir, "nvim");

    let ignored: HashSet<String> = ["node_modules".into()].into();
    let results = scan_home(tmp.path(), &ignored).unwrap();

    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(!names.contains(&"node_modules"));
    assert!(names.contains(&"nvim"));
}

#[test]
fn scan_home_sorted_by_confidence() {
    let tmp = TempDir::new().unwrap();
    create_file(tmp.path(), ".zshrc");
    create_file(tmp.path(), "random.txt");
    create_dir(tmp.path(), ".config");

    let ignored = HashSet::new();
    let results = scan_home(tmp.path(), &ignored).unwrap();

    for window in results.windows(2) {
        assert!(window[0].confidence >= window[1].confidence);
    }
}
