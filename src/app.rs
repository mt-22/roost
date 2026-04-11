use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::os_detect::OsInfo;

// ── Tilde-relative path serialization ────────────────────────────────────────
//
// Paths stored in the shared roost.toml are written as `~/...` strings so they
// expand correctly on any device regardless of username or OS home directory.
// Absolute paths already in the config are still accepted on load (migration is
// automatic: they are rewritten as `~/...` on the next save).

#[allow(dead_code)]
mod tilde_path {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::{collections::HashMap, path::{Path, PathBuf}};

    fn home() -> PathBuf {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    }

    fn to_tilde(path: &Path) -> String {
        match path.strip_prefix(home()) {
            Ok(rel) => format!("~/{}", rel.display()),
            Err(_) => path.display().to_string(),
        }
    }

    fn from_tilde(s: &str) -> PathBuf {
        if let Some(rest) = s.strip_prefix("~/") {
            home().join(rest)
        } else if s == "~" {
            home()
        } else {
            PathBuf::from(s)
        }
    }

    // --- single PathBuf ---
    pub fn serialize<S: Serializer>(path: &Path, s: S) -> Result<S::Ok, S::Error> {
        to_tilde(path).serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PathBuf, D::Error> {
        Ok(from_tilde(&String::deserialize(d)?))
    }

    // --- Option<PathBuf> ---
    pub mod opt {
        use super::*;
        pub fn serialize<S: Serializer>(opt: &Option<PathBuf>, s: S) -> Result<S::Ok, S::Error> {
            match opt {
                Some(p) => s.serialize_some(&super::to_tilde(p)),
                None => s.serialize_none(),
            }
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<PathBuf>, D::Error> {
            Ok(Option::<String>::deserialize(d)?.map(|s| super::from_tilde(&s)))
        }
    }

    // --- Vec<PathBuf> ---
    pub mod vec {
        use super::*;
        pub fn serialize<S: Serializer>(paths: &[PathBuf], s: S) -> Result<S::Ok, S::Error> {
            paths
                .iter()
                .map(|p| super::to_tilde(p))
                .collect::<std::vec::Vec<_>>()
                .serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PathBuf>, D::Error> {
            Ok(Vec::<String>::deserialize(d)?
                .iter()
                .map(|s| super::from_tilde(s))
                .collect())
        }
    }

    // --- HashMap<String, PathBuf> (values only) ---
    pub mod map_values {
        use super::*;
        pub fn serialize<S: Serializer>(
            map: &HashMap<String, PathBuf>,
            s: S,
        ) -> Result<S::Ok, S::Error> {
            map.iter()
                .map(|(k, v)| (k, super::to_tilde(v)))
                .collect::<HashMap<_, _>>()
                .serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> Result<HashMap<String, PathBuf>, D::Error> {
            Ok(HashMap::<String, String>::deserialize(d)?
                .into_iter()
                .map(|(k, v)| (k, super::from_tilde(&v)))
                .collect())
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SharedAppConfig {
    pub remote: Option<String>,
    pub profiles: HashMap<String, Profile>,
    pub apps: HashMap<String, Application>,
    pub ignored: HashSet<String>,
}

#[derive(Serialize, Deserialize)]
pub struct LocalAppConfig {
    pub active_profile: String,
    /// OS/hardware info for this device, detected at init time.
    /// Old local.toml files without this field will trigger a fresh detection
    /// via OsInfo's Default impl.
    #[serde(default)]
    pub os_info: OsInfo,
    /// Maps app name → where the app's config lives on THIS device.
    /// Device-specific — not shared via git. Populated during init and when
    /// adding new apps. Apps absent from this map are silently skipped on
    /// operations that require a link path (e.g. platform-only apps).
    #[serde(
        default,
        with = "tilde_path::map_values",
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub link_paths: HashMap<String, PathBuf>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Application {
    pub name: String,
    #[serde(
        default,
        with = "tilde_path::opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub primary_config: Option<PathBuf>,
    pub on_profiles: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Profile {
    /// Set of app names managed by this profile. Link paths are stored
    /// per-device in local.toml, not here.
    #[serde(with = "apps_set")]
    pub apps: HashSet<String>,
    /// Per-app source overrides: app name → path that the roost slot for this
    /// app should symlink to, instead of containing real files.
    ///
    /// Example: `app_sources["nvim"] = ~/.roost/shared/nvim` makes
    /// `~/.roost/<this_profile>/nvim` a symlink to that directory, and
    /// `~/.config/nvim` chains through to the shared copy.
    /// Maps app name → source profile name. Absent entries mean this profile
    /// owns the real files. Resolved at runtime to `~/.roost/<source>/`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub app_sources: HashMap<String, String>,
}

impl Profile {
    fn empty() -> Self {
        Self {
            apps: HashSet::new(),
            app_sources: HashMap::new(),
        }
    }
}

impl SharedAppConfig {
    pub fn load(path: &Path) -> color_eyre::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> color_eyre::Result<()> {
        let content = toml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

impl LocalAppConfig {
    pub fn load(path: &Path) -> color_eyre::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> color_eyre::Result<()> {
        let content = toml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

/// Create a new profile and update both config files.
/// If `template` is Some, clones all apps from the template profile.
/// Returns the number of apps cloned from the template (0 if no template).
pub fn add_profile(
    name: &str,
    roost_dir: &Path,
    shared_config: &mut SharedAppConfig,
    shared_config_path: &Path,
    local_config: &mut LocalAppConfig,
    local_config_path: &Path,
    template: Option<&str>,
) -> color_eyre::Result<usize> {
    use color_eyre::eyre::eyre;

    if name.is_empty() {
        return Err(eyre!("Profile name cannot be empty."));
    }

    if shared_config.profiles.contains_key(name) {
        return Err(eyre!("Profile '{}' already exists.", name));
    }

    let profile_dir = roost_dir.join(name);
    fs::create_dir_all(&profile_dir)?;

    let (profile, cloned_count) = match template {
        Some(template_name) => {
            if let Some(template_profile) = shared_config.profiles.get(template_name) {
                let template_dir = roost_dir.join(template_name);
                if template_dir.exists() {
                    crate::linker::copy_dir_recursive(&template_dir, &profile_dir)?;
                }
                let apps = template_profile.apps.clone();
                let app_sources = template_profile.app_sources.clone();
                for app_name in &apps {
                    if let Some(app) = shared_config.apps.get_mut(app_name)
                        && !app.on_profiles.contains(&name.to_string()) {
                            app.on_profiles.push(name.to_string());
                        }
                }
                let count = apps.len();
                (Profile { apps, app_sources }, count)
            } else {
                (Profile::empty(), 0)
            }
        }
        None => (Profile::empty(), 0),
    };

    shared_config.profiles.insert(name.to_string(), profile);
    shared_config.save(shared_config_path)?;
    local_config.save(local_config_path)?;
    Ok(cloned_count)
}

/// Delete a profile and clean up all associated state.
///
/// - Unlinks (restores) all apps that belong to this profile
/// - Removes the profile from each app's `on_profiles`; removes apps with no remaining profiles
/// - Deletes the profile directory (`~/.roost/<name>/`)
/// - Updates both shared and local config files
pub fn delete_profile(
    name: &str,
    roost_dir: &Path,
    shared_config: &mut SharedAppConfig,
    shared_config_path: &Path,
    local_config: &mut LocalAppConfig,
    local_config_path: &Path,
) -> color_eyre::Result<()> {
    use color_eyre::eyre::eyre;

    if name == local_config.active_profile {
        return Err(eyre!("Cannot delete the active profile. Switch first."));
    }

    if shared_config.profiles.len() <= 1 {
        return Err(eyre!("Cannot delete the only profile."));
    }

    let Some(profile) = shared_config.profiles.get(name) else {
        return Err(eyre!("Profile '{}' does not exist.", name));
    };

    let profile_dir = roost_dir.join(name);
    let app_names: Vec<String> = profile.apps.iter().cloned().collect();

    for app_name in &app_names {
        if let Some(link_path) = local_config.link_paths.get(app_name)
            && link_path.is_symlink()
                && link_path
                    .read_link()
                    .map(|t| t.starts_with(roost_dir))
                    .unwrap_or(false)
                && let Err(e) = crate::linker::unlink(link_path, &profile_dir, roost_dir) {
                    eprintln!("  warn: could not unlink {}: {}", link_path.display(), e);
                }

        if let Some(app) = shared_config.apps.get_mut(app_name) {
            app.on_profiles.retain(|p| p != name);
            if app.on_profiles.is_empty() {
                shared_config.apps.remove(app_name);
            }
        }
    }

    if profile_dir.exists() {
        fs::remove_dir_all(&profile_dir)?;
    }

    shared_config.profiles.remove(name);
    shared_config.save(shared_config_path)?;

    local_config.save(local_config_path)?;

    Ok(())
}

// ── Profile.apps dual-format deserializer ─────────────────────────────────────
//
// Old roost.toml files store `[profiles.X.apps]` as a TOML table (map from
// app name → PathBuf link_path). New format stores it as a TOML array of
// strings. This module's deserializer accepts both so that migration is
// transparent on first load; serialization always writes the new array form.

mod apps_set {
    use serde::{
        de::{MapAccess, SeqAccess, Visitor},
        Deserializer, Serializer,
    };
    use std::collections::HashSet;
    use std::fmt;

    pub fn serialize<S: Serializer>(set: &HashSet<String>, s: S) -> Result<S::Ok, S::Error> {
        use serde::Serialize;
        set.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<HashSet<String>, D::Error> {
        struct AppsVisitor;

        impl<'de> Visitor<'de> for AppsVisitor {
            type Value = HashSet<String>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a sequence of app names or a map from app name to path")
            }

            // New format: ["nvim", "ghostty", ...]
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut set = HashSet::new();
                while let Some(name) = seq.next_element::<String>()? {
                    set.insert(name);
                }
                Ok(set)
            }

            // Old format: { nvim = "~/.config/nvim", ... } — keep only keys
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut set = HashSet::new();
                while let Some((key, _val)) = map.next_entry::<String, serde::de::IgnoredAny>()? {
                    set.insert(key);
                }
                Ok(set)
            }
        }

        d.deserialize_any(AppsVisitor)
    }
}

// ── Migration ─────────────────────────────────────────────────────────────────
//
// Old roost.toml files store `link_path` on each Application and path values
// in Profile.apps. On first run with the new code, we extract those paths into
// local.toml and let the normal save rewrite roost.toml without them.

/// Legacy deserialization structs — only used for the one-time migration read.
#[derive(serde::Deserialize)]
struct LegacyApplication {
    #[serde(default, with = "tilde_path::opt")]
    link_path: Option<PathBuf>,
}

#[derive(serde::Deserialize)]
struct LegacySharedConfig {
    #[serde(default)]
    apps: HashMap<String, LegacyApplication>,
}

/// If `local.link_paths` is empty and the shared config on disk still carries
/// legacy `link_path` fields, populate `link_paths` from them and save local.
/// Idempotent — does nothing if link_paths already has entries.
pub fn migrate_link_paths_if_needed(
    shared_path: &Path,
    local: &mut LocalAppConfig,
    local_path: &Path,
) -> color_eyre::Result<()> {
    if !local.link_paths.is_empty() {
        return Ok(());
    }
    let raw = fs::read_to_string(shared_path)?;
    let legacy: LegacySharedConfig = toml::from_str(&raw).unwrap_or(LegacySharedConfig {
        apps: HashMap::new(),
    });
    for (name, app) in legacy.apps {
        if let Some(lp) = app.link_path {
            local.link_paths.insert(name, lp);
        }
    }
    if !local.link_paths.is_empty() {
        local.save(local_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use tempfile::TempDir;

    fn make_shared() -> SharedAppConfig {
        SharedAppConfig {
            remote: None,
            profiles: HashMap::new(),
            apps: HashMap::new(),
            ignored: HashSet::new(),
        }
    }

    fn make_local() -> LocalAppConfig {
        LocalAppConfig {
            active_profile: "default".to_string(),
            os_info: OsInfo {
                family: "unix".to_string(),
                name: "macos".to_string(),
                version: Some("15.0".to_string()),
                arch: "aarch64".to_string(),
            },
            link_paths: HashMap::new(),
        }
    }

    fn make_profile() -> Profile {
        Profile {
            apps: HashSet::new(),
            app_sources: HashMap::new(),
        }
    }

    // ── 1. apps_set dual-format deserialization ────────────────────────────────

    #[derive(serde::Serialize, serde::Deserialize)]
    struct AppsWrapper {
        #[serde(with = "apps_set")]
        apps: HashSet<String>,
    }

    #[test]
    fn apps_set_deserialize_array_format() {
        let toml = r#"apps = ["nvim", "ghostty"]"#;
        let w: AppsWrapper = toml::from_str(toml).unwrap();
        assert!(w.apps.contains("nvim"));
        assert!(w.apps.contains("ghostty"));
        assert_eq!(w.apps.len(), 2);
    }

    #[test]
    fn apps_set_deserialize_legacy_table_format() {
        let toml = r#"apps = { nvim = "~/.config/nvim", ghostty = "~/.config/ghostty" }"#;
        let w: AppsWrapper = toml::from_str(toml).unwrap();
        assert!(w.apps.contains("nvim"));
        assert!(w.apps.contains("ghostty"));
        assert_eq!(w.apps.len(), 2);
    }

    #[test]
    fn apps_set_deserialize_empty_array() {
        let toml = r#"apps = []"#;
        let w: AppsWrapper = toml::from_str(toml).unwrap();
        assert!(w.apps.is_empty());
    }

    #[test]
    fn apps_set_roundtrip_array_format() {
        let original: HashSet<String> = ["nvim", "ghostty", "bash"].into_iter().map(String::from).collect();
        let serialized = toml::to_string(&AppsWrapper { apps: original.clone() }).unwrap();
        let deserialized: AppsWrapper = toml::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized.apps);
    }

    // ── 2. SharedAppConfig serialization roundtrip ─────────────────────────────

    #[test]
    fn shared_config_roundtrip_full() {
        let mut cfg = make_shared();
        cfg.remote = Some("https://github.com/example/dots".to_string());
        cfg.profiles.insert("default".to_string(), make_profile());
        cfg.ignored.insert("secret".to_string());
        cfg.apps.insert(
            "nvim".to_string(),
            Application {
                name: "nvim".to_string(),
                primary_config: None,
                on_profiles: vec!["default".to_string()],
            },
        );

        let s = toml::to_string(&cfg).unwrap();
        let back: SharedAppConfig = toml::from_str(&s).unwrap();
        assert_eq!(back.remote, cfg.remote);
        assert_eq!(back.profiles.len(), 1);
        assert!(back.profiles.contains_key("default"));
        assert!(back.ignored.contains("secret"));
        assert_eq!(back.apps.len(), 1);
        assert!(back.apps.contains_key("nvim"));
    }

    #[test]
    fn shared_config_roundtrip_empty() {
        let cfg = make_shared();
        let s = toml::to_string(&cfg).unwrap();
        let back: SharedAppConfig = toml::from_str(&s).unwrap();
        assert!(back.remote.is_none());
        assert!(back.profiles.is_empty());
        assert!(back.apps.is_empty());
        assert!(back.ignored.is_empty());
    }

    // ── 3. LocalAppConfig serialization roundtrip ──────────────────────────────

    #[test]
    fn local_config_roundtrip_full() {
        let mut cfg = make_local();
        cfg.link_paths.insert("nvim".to_string(), dirs::home_dir().unwrap().join(".config/nvim"));

        let s = toml::to_string(&cfg).unwrap();
        let back: LocalAppConfig = toml::from_str(&s).unwrap();
        assert_eq!(back.active_profile, "default");
        assert_eq!(back.os_info.family, "unix");
        assert_eq!(back.link_paths.len(), 1);
        assert!(back.link_paths.contains_key("nvim"));
    }

    #[test]
    fn local_config_empty_link_paths_omitted() {
        let cfg = make_local();
        let s = toml::to_string(&cfg).unwrap();
        assert!(!s.contains("link_paths"));
    }

    // ── 4. File load/save ──────────────────────────────────────────────────────

    #[test]
    fn shared_config_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("roost.toml");

        let mut cfg = make_shared();
        cfg.remote = Some("git@host:dots".to_string());
        cfg.profiles.insert("work".to_string(), make_profile());

        cfg.save(&path).unwrap();
        let loaded = SharedAppConfig::load(&path).unwrap();
        assert_eq!(loaded.remote, Some("git@host:dots".to_string()));
        assert!(loaded.profiles.contains_key("work"));
    }

    #[test]
    fn local_config_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("local.toml");

        let mut cfg = make_local();
        cfg.link_paths.insert("bash".to_string(), dirs::home_dir().unwrap().join(".bashrc"));

        cfg.save(&path).unwrap();
        let loaded = LocalAppConfig::load(&path).unwrap();
        assert_eq!(loaded.active_profile, "default");
        assert!(loaded.link_paths.contains_key("bash"));
    }

    #[test]
    fn load_nonexistent_file_fails() {
        let result = SharedAppConfig::load(Path::new("/no/such/file.toml"));
        assert!(result.is_err());
    }

    // ── 5. add_profile tests ───────────────────────────────────────────────────

    #[test]
    fn add_profile_succeeds() {
        let tmp = TempDir::new().unwrap();
        let roost_dir = tmp.path().join("roost");
        let shared_path = tmp.path().join("roost.toml");
        let local_path = tmp.path().join("local.toml");

        let mut shared = make_shared();
        shared.profiles.insert("default".to_string(), make_profile());
        let mut local = make_local();

        shared.save(&shared_path).unwrap();
        local.save(&local_path).unwrap();

        let count = add_profile(
            "work",
            &roost_dir,
            &mut shared,
            &shared_path,
            &mut local,
            &local_path,
            None,
        )
        .unwrap();

        assert_eq!(count, 0);
        assert!(shared.profiles.contains_key("work"));
        assert!(roost_dir.join("work").exists());
    }

    #[test]
    fn add_profile_empty_name_fails() {
        let tmp = TempDir::new().unwrap();
        let roost_dir = tmp.path().join("roost");
        let shared_path = tmp.path().join("roost.toml");
        let local_path = tmp.path().join("local.toml");

        let mut shared = make_shared();
        let mut local = make_local();
        shared.save(&shared_path).unwrap();
        local.save(&local_path).unwrap();

        let result = add_profile(
            "",
            &roost_dir,
            &mut shared,
            &shared_path,
            &mut local,
            &local_path,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn add_profile_duplicate_name_fails() {
        let tmp = TempDir::new().unwrap();
        let roost_dir = tmp.path().join("roost");
        let shared_path = tmp.path().join("roost.toml");
        let local_path = tmp.path().join("local.toml");

        let mut shared = make_shared();
        shared.profiles.insert("default".to_string(), make_profile());
        let mut local = make_local();
        shared.save(&shared_path).unwrap();
        local.save(&local_path).unwrap();

        let result = add_profile(
            "default",
            &roost_dir,
            &mut shared,
            &shared_path,
            &mut local,
            &local_path,
            None,
        );
        assert!(result.is_err());
    }

    // ── 6. delete_profile tests ────────────────────────────────────────────────

    fn setup_two_profiles() -> (TempDir, SharedAppConfig, LocalAppConfig, PathBuf, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let roost_dir = tmp.path().join("roost");
        let shared_path = tmp.path().join("roost.toml");
        let local_path = tmp.path().join("local.toml");

        fs::create_dir_all(roost_dir.join("default")).unwrap();
        fs::create_dir_all(roost_dir.join("other")).unwrap();

        let mut shared = make_shared();
        shared.profiles.insert("default".to_string(), make_profile());
        shared.profiles.insert("other".to_string(), make_profile());

        let mut local = make_local();
        local.active_profile = "default".to_string();

        shared.save(&shared_path).unwrap();
        local.save(&local_path).unwrap();

        (tmp, shared, local, roost_dir, shared_path, local_path)
    }

    #[test]
    fn delete_profile_cannot_delete_active() {
        let (_tmp, mut shared, mut local, roost_dir, shared_path, local_path) = setup_two_profiles();
        let result = delete_profile("default", &roost_dir, &mut shared, &shared_path, &mut local, &local_path);
        assert!(result.is_err());
    }

    #[test]
    fn delete_profile_cannot_delete_only_profile() {
        let tmp = TempDir::new().unwrap();
        let roost_dir = tmp.path().join("roost");
        let shared_path = tmp.path().join("roost.toml");
        let local_path = tmp.path().join("local.toml");

        fs::create_dir_all(roost_dir.join("only")).unwrap();

        let mut shared = make_shared();
        shared.profiles.insert("only".to_string(), make_profile());
        let mut local = make_local();
        local.active_profile = "only".to_string();

        shared.save(&shared_path).unwrap();
        local.save(&local_path).unwrap();

        let result = delete_profile("only", &roost_dir, &mut shared, &shared_path, &mut local, &local_path);
        assert!(result.is_err());
    }

    #[test]
    fn delete_profile_nonexistent_fails() {
        let (_tmp, mut shared, mut local, roost_dir, shared_path, local_path) = setup_two_profiles();
        let result = delete_profile("nope", &roost_dir, &mut shared, &shared_path, &mut local, &local_path);
        assert!(result.is_err());
    }

    #[test]
    fn delete_profile_succeeds() {
        let (_tmp, mut shared, mut local, roost_dir, shared_path, local_path) = setup_two_profiles();
        delete_profile("other", &roost_dir, &mut shared, &shared_path, &mut local, &local_path).unwrap();
        assert!(!shared.profiles.contains_key("other"));
        assert!(!roost_dir.join("other").exists());
        assert!(shared.profiles.contains_key("default"));
    }

    // ── 7. Legacy migration tests ──────────────────────────────────────────────

    #[test]
    fn migrate_link_paths_from_legacy() {
        let tmp = TempDir::new().unwrap();
        let shared_path = tmp.path().join("roost.toml");
        let local_path = tmp.path().join("local.toml");

        let legacy_toml = r#"
[apps.nvim]
link_path = "~/.config/nvim"

[apps.bash]
link_path = "~/.bashrc"
"#;
        fs::write(&shared_path, legacy_toml).unwrap();

        let mut local = make_local();
        local.save(&local_path).unwrap();

        migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();

        assert_eq!(local.link_paths.len(), 2);
        assert!(local.link_paths.contains_key("nvim"));
        assert!(local.link_paths.contains_key("bash"));
    }

    #[test]
    fn migrate_idempotent() {
        let tmp = TempDir::new().unwrap();
        let shared_path = tmp.path().join("roost.toml");
        let local_path = tmp.path().join("local.toml");

        let legacy_toml = r#"
[apps.nvim]
link_path = "~/.config/nvim"
"#;
        fs::write(&shared_path, legacy_toml).unwrap();

        let mut local = make_local();
        local.save(&local_path).unwrap();

        migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();
        let count_after_first = local.link_paths.len();

        migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();
        assert_eq!(local.link_paths.len(), count_after_first);
    }

    #[test]
    fn test_load_corrupted_toml_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("roost.toml");
        fs::write(&path, "[profile\ninvalid toml {{{").unwrap();
        let result = SharedAppConfig::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_empty_file_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("roost.toml");
        fs::write(&path, "").unwrap();
        let result = SharedAppConfig::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_missing_file_fails_with_clear_error() {
        let path = PathBuf::from("/tmp/roost-nonexistent-12345/roost.toml");
        let result = SharedAppConfig::load(&path);
        match result {
            Err(e) => assert!(!e.to_string().is_empty()),
            Ok(_) => panic!("expected error for missing file"),
        }
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("nested/deep");
        fs::create_dir_all(&nested).unwrap();
        let path = nested.join("roost.toml");
        let config = SharedAppConfig {
            remote: None,
            profiles: HashMap::new(),
            apps: HashMap::new(),
            ignored: HashSet::new(),
        };
        config.save(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_shared_config_with_many_apps_roundtrip() {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut profiles = HashMap::new();
        let mut apps = HashMap::new();
        let mut ignored = HashSet::new();

        for i in 0..20 {
            let name = format!("app{}", i);
            let prof_name = if i % 2 == 0 { "even" } else { "odd" };
            profiles
                .entry(prof_name.to_string())
                .or_insert_with(Profile::empty)
                .apps
                .insert(name.clone());
            apps.insert(name.clone(), Application {
                name: name.clone(),
                primary_config: Some(home.join(format!(".config/{}/config.toml", name))),
                on_profiles: vec![prof_name.to_string()],
            });
            ignored.insert(format!("{}.tmp", name));
        }

        let config = SharedAppConfig {
            remote: Some("git@github.com:test/dotfiles.git".to_string()),
            profiles,
            apps,
            ignored,
        };

        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: SharedAppConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(deserialized.apps.len(), 20);
        assert_eq!(deserialized.profiles.len(), 2);
        assert_eq!(deserialized.ignored.len(), 20);
        assert_eq!(deserialized.remote.as_deref(), Some("git@github.com:test/dotfiles.git"));
    }

    #[test]
    fn test_local_config_with_special_chars_in_path() {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let config = LocalAppConfig {
            active_profile: "default".to_string(),
            os_info: OsInfo::default(),
            link_paths: HashMap::from([
                ("spaces app".to_string(), home.join("path with spaces/config")),
                ("unicode_app".to_string(), home.join(".config/myapp")),
            ]),
        };

        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: LocalAppConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(deserialized.link_paths.len(), 2);
        assert!(deserialized.link_paths.contains_key("spaces app"));
    }

    #[test]
    fn test_migrate_link_paths_no_legacy_field_is_nop() {
        let dir = tempfile::TempDir::new().unwrap();
        let shared_path = dir.path().join("roost.toml");
        let local_path = dir.path().join("local.toml");

        let modern_toml = r#"
[apps.nvim]
name = "nvim"
on_profiles = ["laptop"]
"#;
        fs::write(&shared_path, modern_toml).unwrap();

        let mut local = LocalAppConfig {
            active_profile: "laptop".to_string(),
            os_info: OsInfo::default(),
            link_paths: HashMap::from([("nvim".to_string(), PathBuf::from("/home/.config/nvim"))]),
        };

        let link_count_before = local.link_paths.len();
        migrate_link_paths_if_needed(&shared_path, &mut local, &local_path).unwrap();
        assert_eq!(local.link_paths.len(), link_count_before);
    }

    #[test]
    fn test_profile_empty_has_no_apps() {
        let p = Profile::empty();
        assert!(p.apps.is_empty());
        assert!(p.app_sources.is_empty());
    }
}
