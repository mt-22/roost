use crate::os_detect::OsInfo;
use color_eyre::Result;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Serialize a PathBuf as a tilde-prefixed string when it lives inside the home directory.
fn serialize_tilde<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match path {
        None => serializer.serialize_none(),
        Some(p) => {
            if let Some(home) = dirs::home_dir() {
                if let Ok(stripped) = p.strip_prefix(&home) {
                    let tilde_path = PathBuf::from("~").join(stripped);
                    serializer.serialize_some(&tilde_path.display().to_string())
                } else {
                    serializer.serialize_some(&p.display().to_string())
                }
            } else {
                serializer.serialize_some(&p.display().to_string())
            }
        }
    }
}

/// Deserialize a string path, expanding a leading `~/` to the current home directory.
fn deserialize_tilde<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    let maybe_str: Option<String> = Option::deserialize(deserializer)?;
    match maybe_str {
        None => Ok(None),
        Some(s) => {
            if let Some(rest) = s.strip_prefix("~/") {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                Ok(Some(home.join(rest)))
            } else {
                Ok(Some(PathBuf::from(s)))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedAppConfig {
    pub remote: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
    #[serde(default)]
    pub apps: BTreeMap<String, Application>,
    #[serde(default)]
    pub ignored: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAppConfig {
    pub active_profile: String,
    pub os_info: OsInfo,
    #[serde(default)]
    pub link_paths: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub apps: BTreeSet<String>,
    #[serde(default)]
    pub app_sources: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
    #[serde(
        default,
        serialize_with = "serialize_tilde",
        deserialize_with = "deserialize_tilde"
    )]
    pub primary_config: Option<PathBuf>,
    #[serde(default)]
    pub on_profiles: BTreeSet<String>,
    pub is_dir: bool,
    #[serde(default)]
    pub ignore: Vec<String>,
}

pub fn roost_dir() -> PathBuf {
    std::env::var("ROOST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("could not determine home directory")
                .join(".roost")
        })
}

pub fn profile_dir(roost_dir: &Path, profile_name: &str) -> PathBuf {
    roost_dir.join(profile_name)
}

pub fn shared_config_path(roost_dir: &Path) -> PathBuf {
    roost_dir.join("roost.toml")
}

pub fn local_config_path(roost_dir: &Path) -> PathBuf {
    roost_dir.join("local.toml")
}

pub fn load_shared(path: &PathBuf) -> Result<SharedAppConfig> {
    let raw = std::fs::read_to_string(path)?;
    let config: SharedAppConfig = toml::from_str(&raw)?;
    validate_shared(&config)?;
    Ok(config)
}

pub fn save_shared(path: &PathBuf, config: &SharedAppConfig) -> Result<()> {
    let contents = toml::to_string_pretty(config)?;
    atomic_write(path, &contents)?;
    Ok(())
}

pub fn load_local(path: &PathBuf) -> Result<LocalAppConfig> {
    let raw = std::fs::read_to_string(path)?;
    let config: LocalAppConfig = toml::from_str(&raw)?;
    Ok(config)
}

pub fn save_local(path: &PathBuf, config: &LocalAppConfig) -> Result<()> {
    let contents = toml::to_string_pretty(config)?;
    atomic_write(path, &contents)?;
    Ok(())
}

/// Atomically write a file by writing to a .tmp sibling then renaming.
/// This prevents corruption if the process crashes mid-write.
fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// For apps with exactly one file, automatically set `primary_config`
/// so the user can press `o` in the TUI without an extra step.
pub fn guess_primary_configs(
    roost_dir: &Path,
    profile_name: &str,
    config: &mut SharedAppConfig,
    local: &LocalAppConfig,
) -> Result<()> {
    for (app_name, app) in config.apps.iter_mut() {
        if app.primary_config.is_some() {
            continue;
        }
        let Some(original_base) = local.link_paths.get(app_name) else {
            continue;
        };
        if !app.is_dir {
            // Single-file app (stored in misc/): the app itself is the primary config.
            app.primary_config = Some(original_base.clone());
        } else {
            let app_dir = roost_dir.join(profile_name).join(app_name);
            let entries: Vec<_> = match std::fs::read_dir(&app_dir) {
                Ok(it) => it.filter_map(|e| e.ok()).collect(),
                Err(_) => continue,
            };
            if entries.len() == 1 {
                if let Some(name) = entries[0].file_name().to_str() {
                    app.primary_config = Some(original_base.join(name));
                }
            }
        }
    }
    Ok(())
}

pub fn validate_shared(config: &SharedAppConfig) -> Result<()> {
    // Check that apps referenced by profiles exist in the apps map
    for (profile_name, profile) in &config.profiles {
        for app_name in &profile.apps {
            if !config.apps.contains_key(app_name) {
                color_eyre::eyre::bail!(
                    "profile '{}' references unknown app '{}'",
                    profile_name,
                    app_name
                );
            }
        }
        // Check for cycles in app_sources
        for (app_name, source_profile) in &profile.app_sources {
            if !config.apps.contains_key(app_name) {
                color_eyre::eyre::bail!(
                    "profile '{}' has source for unknown app '{}'",
                    profile_name,
                    app_name
                );
            }
            if !config.profiles.contains_key(source_profile) {
                color_eyre::eyre::bail!(
                    "profile '{}' sources app '{}' from unknown profile '{}'",
                    profile_name,
                    app_name,
                    source_profile
                );
            }
            // Detect direct cycle: A sources from B, and B sources A back
            if let Some(source) = config.profiles.get(source_profile)
                && let Some(back_ref) = source.app_sources.get(app_name)
                    && back_ref == profile_name {
                        color_eyre::eyre::bail!(
                            "cycle detected: app '{}' between profiles '{}' and '{}'",
                            app_name,
                            profile_name,
                            source_profile
                        );
                    }
        }
    }
    // Check that on_profiles in apps reference real profiles
    for (app_name, app) in &config.apps {
        for profile_name in &app.on_profiles {
            if !config.profiles.contains_key(profile_name) {
                color_eyre::eyre::bail!(
                    "app '{}' references unknown profile '{}'",
                    app_name,
                    profile_name
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
