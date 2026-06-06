use color_eyre::Result;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use crate::app::Application;

const HEADER: &str = "# === Roost-managed begin ===";
const FOOTER: &str = "# === Roost-managed end ===";

/// Regenerate the root `.gitignore` at `roost_dir` from global patterns and
/// per-app ignore rules.  User-written rules outside the managed block are
/// preserved.
pub fn regenerate(
    roost_dir: &Path,
    global_patterns: &BTreeSet<String>,
    apps: &BTreeMap<String, Application>,
) -> Result<()> {
    let path = roost_dir.join(".gitignore");

    let user_rules = if path.exists() {
        let content = fs::read_to_string(&path)?;
        extract_user_rules(&content)
    } else {
        String::new()
    };

    let mut lines: Vec<String> = vec![
        HEADER.to_string(),
        "local.toml".to_string(),
        ".backups/".to_string(),
        "*.local".to_string(),
    ];

    // Global patterns
    if !global_patterns.is_empty() {
        lines.push("# global".to_string());
        for p in global_patterns {
            lines.push(translate_pattern(p));
        }
    }

    // Per-app patterns (only for directory apps)
    let mut app_lines: Vec<String> = Vec::new();
    for (name, app) in apps {
        if app.is_dir && !app.ignore.is_empty() {
            app_lines.push(format!("# app: {}", name));
            for profile in &app.on_profiles {
                for p in &app.ignore {
                    app_lines.push(format!("{}/{}/{}", profile, name, p));
                }
            }
        }
    }
    if !app_lines.is_empty() {
        lines.push("# per-app".to_string());
        lines.extend(app_lines);
    }

    lines.push(FOOTER.to_string());

    let output = if user_rules.trim().is_empty() {
        lines.join("\n") + "\n"
    } else {
        lines.join("\n") + "\n\n" + &user_rules + "\n"
    };

    // atomic write: unique temp file then rename prevents mid-write corruption
    // and avoids clobbering concurrent roost processes
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    let tmp = path.with_extension(format!("tmp.{pid}.{now}"));
    fs::write(&tmp, output)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn extract_user_rules(content: &str) -> String {
    let mut in_managed = false;
    let mut user_lines: Vec<&str> = Vec::new();
    for line in content.lines() {
        if line.trim() == HEADER {
            in_managed = true;
            continue;
        }
        if line.trim() == FOOTER {
            in_managed = false;
            continue;
        }
        if !in_managed {
            user_lines.push(line);
        }
    }
    // Trim leading/trailing blank lines but preserve internal ones
    let trimmed: Vec<&str> = user_lines
        .iter()
        .skip_while(|l| l.trim().is_empty())
        .cloned()
        .collect();
    // Remove trailing blanks
    let mut result: Vec<&str> = trimmed;
    while result.last().map(|l| l.trim().is_empty()) == Some(true) {
        result.pop();
    }
    result.join("\n")
}

fn translate_pattern(p: &str) -> String {
    // `.git` inside an ingested app directory should be ignored, but we must
    // not match the roost repo's own `.git` directory.  Using `**/.git/`
    // achieves this safely.
    if p == ".git" {
        "**/.git/".to_string()
    } else {
        p.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn make_app(ignore: Vec<&str>, is_dir: bool) -> Application {
        let mut on_profiles = BTreeSet::new();
        on_profiles.insert("default".to_string());
        Application {
            primary_config: None,
            on_profiles,
            is_dir,
            ignore: ignore.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn generates_basic_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = BTreeMap::new();
        let globals = BTreeSet::new();
        regenerate(tmp.path(), &globals, &apps).unwrap();

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("local.toml"));
        assert!(content.contains(".backups/"));
        assert!(content.contains("*.local"));
        assert!(content.contains(HEADER));
        assert!(content.contains(FOOTER));
    }

    #[test]
    fn includes_global_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = BTreeMap::new();
        let mut globals = BTreeSet::new();
        globals.insert("node_modules".to_string());
        globals.insert("*.log".to_string());
        regenerate(tmp.path(), &globals, &apps).unwrap();

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules"));
        assert!(content.contains("*.log"));
    }

    #[test]
    fn translates_git_to_safe_form() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = BTreeMap::new();
        let mut globals = BTreeSet::new();
        globals.insert(".git".to_string());
        regenerate(tmp.path(), &globals, &apps).unwrap();

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("**/.git/"));
        assert!(!content.contains("\n.git\n"));
    }

    #[test]
    fn includes_per_app_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        let mut apps = BTreeMap::new();
        apps.insert("sketchybar".to_string(), make_app(vec!["clipboard*"], true));
        let globals = BTreeSet::new();
        regenerate(tmp.path(), &globals, &apps).unwrap();

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("default/sketchybar/clipboard*"));
    }

    #[test]
    fn skips_file_apps_for_per_app_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let mut apps = BTreeMap::new();
        apps.insert(
            "zshrc".to_string(),
            make_app(vec!["should-not-appear"], false),
        );
        let globals = BTreeSet::new();
        regenerate(tmp.path(), &globals, &apps).unwrap();

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(!content.contains("should-not-appear"));
    }

    #[test]
    fn preserves_user_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let existing = format!("{HEADER}\nlocal.toml\n{FOOTER}\n\n# my custom rule\nsecrets.txt\n");
        fs::write(tmp.path().join(".gitignore"), &existing).unwrap();

        let mut globals = BTreeSet::new();
        globals.insert("node_modules".to_string());
        let apps = BTreeMap::new();
        regenerate(tmp.path(), &globals, &apps).unwrap();

        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules"));
        assert!(content.contains("secrets.txt"));
        assert!(content.contains("# my custom rule"));
    }

    #[test]
    fn regenerate_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let mut globals = BTreeSet::new();
        globals.insert("*.log".to_string());
        let apps = BTreeMap::new();

        regenerate(tmp.path(), &globals, &apps).unwrap();
        let first = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();

        regenerate(tmp.path(), &globals, &apps).unwrap();
        let second = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();

        assert_eq!(first, second);
    }
}
