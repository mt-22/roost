use crate::os_detect::OsInfo;
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedAppConfig {
    pub remote: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub apps: HashMap<String, Application>,
    #[serde(default)]
    pub ignored: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAppConfig {
    pub active_profile: String,
    pub os_info: OsInfo,
    #[serde(default)]
    pub link_paths: HashMap<String, PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub apps: HashSet<String>,
    #[serde(default)]
    pub app_sources: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
    pub primary_config: Option<PathBuf>,
    #[serde(default)]
    pub on_profiles: HashSet<String>,
    pub is_dir: bool,
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

pub fn shared_config_path(roost_dir: &PathBuf) -> PathBuf {
    roost_dir.join("roost.toml")
}

pub fn local_config_path(roost_dir: &PathBuf) -> PathBuf {
    roost_dir.join("local.toml")
}

pub fn load_shared(path: &PathBuf) -> Result<SharedAppConfig> {
    let raw = std::fs::read_to_string(path)?;
    let migrated = migrate_shared(&raw);
    let config: SharedAppConfig = toml::from_str(&migrated)?;
    validate_shared(&config)?;
    Ok(config)
}

pub fn save_shared(path: &PathBuf, config: &SharedAppConfig) -> Result<()> {
    let contents = toml::to_string_pretty(config)?;
    std::fs::write(path, contents)?;
    Ok(())
}

pub fn load_local(path: &PathBuf) -> Result<LocalAppConfig> {
    let raw = std::fs::read_to_string(path)?;
    let config: LocalAppConfig = toml::from_str(&raw)?;
    Ok(config)
}

pub fn save_local(path: &PathBuf, config: &LocalAppConfig) -> Result<()> {
    let contents = toml::to_string_pretty(config)?;
    std::fs::write(path, contents)?;
    Ok(())
}

fn migrate_shared(raw: &str) -> String {
    let doc = raw.to_string();
    // Future migration hooks go here.
    // e.g. old `apps` list format -> table format
    // e.g. old `link_path` -> `link_paths`
    doc
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
            if let Some(source) = config.profiles.get(source_profile) {
                if let Some(back_ref) = source.app_sources.get(app_name) {
                    if back_ref == profile_name {
                        color_eyre::eyre::bail!(
                            "cycle detected: app '{}' between profiles '{}' and '{}'",
                            app_name,
                            profile_name,
                            source_profile
                        );
                    }
                }
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
