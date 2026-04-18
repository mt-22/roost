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
mod tests;
