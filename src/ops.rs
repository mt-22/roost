use color_eyre::{Result, eyre::bail};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::app::{
    Application, LocalAppConfig, Profile, SharedAppConfig, save_local, save_shared,
    shared_config_path, local_config_path, profile_dir, validate_app_name, validate_profile_name,
};
use crate::linker;

/// Result of an add-app operation.
pub struct AddAppResult {
    pub app_name: String,
    pub link_actions: Vec<String>,
}

/// Ingest a config path into the active profile.
///
/// Validates the path, moves the origin into roost, creates a symlink,
/// updates shared and local configs, ensures links, and regenerates .gitignore.
pub fn add_app(
    origin: &Path,
    app_name: &str,
    is_dir: bool,
    shared: &mut SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<AddAppResult> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    linker::validate_path_in_home(origin, &home)?;
    validate_app_name(app_name)?;

    let profile_name = local.active_profile.clone();
    validate_profile_name(&profile_name)?;
    let pdir = profile_dir(roost_dir, &profile_name);

    if !origin.exists() {
        bail!("Path '{}' does not exist.", origin.display());
    }

    if shared.apps.contains_key(app_name) {
        bail!("App '{}' already managed.", app_name);
    }

    linker::ingest(origin, &pdir, app_name, is_dir)?;

    shared.apps.insert(
        app_name.to_string(),
        Application {
            primary_config: None,
            on_profiles: {
                let mut s = BTreeSet::new();
                s.insert(profile_name.clone());
                s
            },
            is_dir,
            ignore: Vec::new(),
        },
    );

    if let Some(profile) = shared.profiles.get_mut(&profile_name) {
        profile.apps.insert(app_name.to_string());
    }

    local
        .link_paths
        .insert(app_name.to_string(), origin.to_path_buf());

    save_shared(&shared_config_path(roost_dir), shared)?;
    save_local(&local_config_path(roost_dir), local)?;

    let actions = linker::ensure_links(shared, local, roost_dir)?;

    crate::gitignore::regenerate(roost_dir, &shared.ignored, &shared.apps)?;

    Ok(AddAppResult {
        app_name: app_name.to_string(),
        link_actions: actions,
    })
}

/// Remove an app from the active profile. If no other profile references it,
/// restore files to the origin. Otherwise just remove the profile reference.
pub fn remove_app(
    app_name: &str,
    shared: &mut SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<()> {
    validate_app_name(app_name)?;

    let profile_name = local.active_profile.clone();
    validate_profile_name(&profile_name)?;

    // Extract needed data before mutable borrows
    let is_dir = shared
        .apps
        .get(app_name)
        .map(|a| a.is_dir)
        .unwrap_or(true);
    let origin = local
        .link_paths
        .get(app_name)
        .cloned()
        .ok_or_else(|| color_eyre::eyre::eyre!("No link path for '{}'.", app_name))?;

    // Remove from current profile
    if let Some(profile) = shared.profiles.get_mut(&profile_name) {
        profile.app_sources.remove(app_name);
        profile.apps.remove(app_name);
    }

    if let Some(app_entry) = shared.apps.get_mut(app_name) {
        app_entry.on_profiles.remove(&profile_name);
    }

    // Check if any other profile still references this app
    let other_profiles_have_it = shared
        .profiles
        .values()
        .any(|p| p.apps.contains(app_name));

    if other_profiles_have_it {
        save_shared(&shared_config_path(roost_dir), shared)?;
        return Ok(());
    }

    // This was the last profile — fully remove the app
    let pdir = profile_dir(roost_dir, &profile_name);
    linker::unlink(&origin, &pdir, app_name, is_dir)?;

    shared.apps.remove(app_name);
    local.link_paths.remove(app_name);

    save_shared(&shared_config_path(roost_dir), shared)?;
    save_local(&local_config_path(roost_dir), local)?;

    Ok(())
}

/// Create a new profile.
pub fn create_profile(
    name: &str,
    copy_from: Option<&str>,
    shared: &mut SharedAppConfig,
    _local: &LocalAppConfig,
    roost_dir: &Path,
) -> Result<()> {
    validate_profile_name(name)?;

    if shared.profiles.contains_key(name) {
        bail!("Profile '{}' already exists.", name);
    }

    let new_profile = if let Some(source_name) = copy_from {
        validate_profile_name(source_name)?;
        let source = shared
            .profiles
            .get(source_name)
            .ok_or_else(|| color_eyre::eyre::eyre!("Source profile '{}' not found.", source_name))?;

        // Physically copy app files into the new profile directory.
        let source_profile_dir = profile_dir(roost_dir, source_name);
        let target_profile_dir = profile_dir(roost_dir, name);
        std::fs::create_dir_all(&target_profile_dir)?;

        for app_name in &source.apps {
            let is_dir = shared
                .apps
                .get(app_name)
                .map(|a| a.is_dir)
                .unwrap_or(true);
            let source_path = linker::app_dest(&source_profile_dir, app_name, is_dir);
            let target_path = linker::app_dest(&target_profile_dir, app_name, is_dir);
            if source_path.exists() && !target_path.exists() {
                if is_dir {
                    linker::copy_dir_recursive(&source_path, &target_path)?;
                } else {
                    if let Some(parent) = target_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(&source_path, &target_path)?;
                }
            }
        }

        Profile {
            apps: source.apps.clone(),
            app_sources: source.app_sources.clone(),
        }
    } else {
        Profile {
            apps: BTreeSet::new(),
            app_sources: BTreeMap::new(),
        }
    };

    shared.profiles.insert(name.to_string(), new_profile);
    save_shared(&shared_config_path(roost_dir), shared)?;

    Ok(())
}

/// Delete a profile from config.
pub fn delete_profile(
    name: &str,
    shared: &mut SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<Option<String>> {
    validate_profile_name(name)?;

    if !shared.profiles.contains_key(name) {
        bail!("Profile '{}' does not exist.", name);
    }

    for app_name in shared.profiles[name].apps.iter() {
        if let Some(app) = shared.apps.get_mut(app_name) {
            app.on_profiles.remove(name);
            if app.on_profiles.is_empty() {
                shared.apps.remove(app_name);
            }
        }
    }
    shared.profiles.remove(name);

    let mut fallback_msg = None;
    if local.active_profile == name {
        let fallback = shared
            .profiles
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        local.active_profile = fallback.clone();
        save_local(&local_config_path(roost_dir), local)?;
        fallback_msg = Some(fallback);
    }

    save_shared(&shared_config_path(roost_dir), shared)?;

    // Remove the profile's directory from disk
    let profile_dir = profile_dir(roost_dir, name);
    if profile_dir.exists() {
        fs::remove_dir_all(&profile_dir)?;
    }

    Ok(fallback_msg)
}

/// Rename a profile.
pub fn rename_profile(
    old: &str,
    new: &str,
    shared: &mut SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<()> {
    validate_profile_name(old)?;
    validate_profile_name(new)?;

    if !shared.profiles.contains_key(old) {
        bail!("Profile '{}' does not exist.", old);
    }
    if shared.profiles.contains_key(new) {
        bail!("Profile '{}' already exists.", new);
    }

    let profile = shared.profiles.remove(old).unwrap();
    shared.profiles.insert(new.to_string(), profile);

    for app in shared.apps.values_mut() {
        if app.on_profiles.remove(old) {
            app.on_profiles.insert(new.to_string());
        }
    }

    for profile in shared.profiles.values_mut() {
        let updates: Vec<(String, String)> = profile
            .app_sources
            .iter()
            .filter(|(_, src)| *src == old)
            .map(|(app_name, _)| (app_name.clone(), new.to_string()))
            .collect();
        for (app_name, new_src) in updates {
            profile.app_sources.insert(app_name, new_src);
        }
    }

    if local.active_profile == old {
        local.active_profile = new.to_string();
        save_local(&local_config_path(roost_dir), local)?;
    }

    save_shared(&shared_config_path(roost_dir), shared)?;

    Ok(())
}

/// Import an app from another profile via symlink (zero-copy).
pub struct ImportAppResult {
    pub app_name: String,
    pub source_profile: String,
    pub link_actions: Vec<String>,
}

pub fn import_app(
    app_name: &str,
    source_profile: &str,
    shared: &mut SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<ImportAppResult> {
    validate_app_name(app_name)?;
    validate_profile_name(source_profile)?;

    let current_profile = local.active_profile.clone();
    validate_profile_name(&current_profile)?;

    if !shared.profiles.contains_key(source_profile) {
        bail!("Source profile '{}' not found.", source_profile);
    }
    if current_profile == source_profile {
        bail!("Cannot import from the same profile.");
    }
    if !shared.profiles[source_profile].apps.contains(app_name) {
        bail!(
            "App '{}' not found in profile '{}'.",
            app_name,
            source_profile
        );
    }
    if shared
        .profiles
        .get(&current_profile)
        .map(|p| p.apps.contains(app_name))
        .unwrap_or(false)
    {
        bail!(
            "App '{}' is already in profile '{}'.",
            app_name,
            current_profile
        );
    }

    linker::import_from(app_name, source_profile, &current_profile, roost_dir)?;

    if let Some(profile) = shared.profiles.get_mut(&current_profile) {
        profile.apps.insert(app_name.to_string());
        profile
            .app_sources
            .insert(app_name.to_string(), source_profile.to_string());
    }

    if let Some(app_entry) = shared.apps.get_mut(app_name) {
        app_entry.on_profiles.insert(current_profile.clone());
    }

    save_shared(&shared_config_path(roost_dir), shared)?;

    let actions = linker::ensure_links(shared, local, roost_dir)?;
    save_local(&local_config_path(roost_dir), local)?;

    Ok(ImportAppResult {
        app_name: app_name.to_string(),
        source_profile: source_profile.to_string(),
        link_actions: actions,
    })
}

/// Copy an app to another profile (physical copy).
pub struct CopyAppResult {
    pub app_name: String,
    pub target_profile: String,
}

pub fn copy_app(
    app_name: &str,
    target_profile: &str,
    shared: &mut SharedAppConfig,
    local: &LocalAppConfig,
    roost_dir: &Path,
) -> Result<CopyAppResult> {
    validate_app_name(app_name)?;
    validate_profile_name(target_profile)?;

    let current_profile = local.active_profile.clone();
    validate_profile_name(&current_profile)?;

    if !shared.profiles.contains_key(target_profile) {
        bail!("Target profile '{}' not found.", target_profile);
    }
    if current_profile == target_profile {
        bail!("Cannot copy to the same profile.");
    }
    if !shared
        .profiles
        .get(&current_profile)
        .map(|p| p.apps.contains(app_name))
        .unwrap_or(false)
    {
        bail!(
            "App '{}' not found in active profile '{}'.",
            app_name,
            current_profile
        );
    }
    if shared
        .profiles
        .get(target_profile)
        .map(|p| p.apps.contains(app_name))
        .unwrap_or(false)
    {
        bail!(
            "App '{}' is already in profile '{}'.",
            app_name,
            target_profile
        );
    }

    linker::copy_to(app_name, &current_profile, target_profile, roost_dir)?;

    if let Some(profile) = shared.profiles.get_mut(target_profile) {
        profile.apps.insert(app_name.to_string());
    }

    if let Some(app_entry) = shared.apps.get_mut(app_name) {
        app_entry.on_profiles.insert(target_profile.to_string());
    }

    save_shared(&shared_config_path(roost_dir), shared)?;

    Ok(CopyAppResult {
        app_name: app_name.to_string(),
        target_profile: target_profile.to_string(),
    })
}

/// Set the primary config path for an app.
pub fn set_primary(
    app_name: &str,
    path: &Path,
    source_profile: Option<&str>,
    shared: &mut SharedAppConfig,
    local: &LocalAppConfig,
    roost_dir: &Path,
) -> Result<()> {
    validate_app_name(app_name)?;

    let app_entry = shared
        .apps
        .get_mut(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;

    let resolved = if let Some(original_base) = local.link_paths.get(app_name) {
        let profile_base = if let Some(src) = source_profile {
            profile_dir(roost_dir, src)
        } else {
            profile_dir(roost_dir, &local.active_profile)
        };
        let app_dir = if app_entry.is_dir {
            profile_base.join(app_name)
        } else {
            profile_base.join(crate::linker::MISC_DIR_NAME)
        };
        if path.starts_with(&app_dir) {
            if let Ok(rel) = path.strip_prefix(&app_dir) {
                original_base.join(rel)
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    app_entry.primary_config = Some(resolved);
    save_shared(&shared_config_path(roost_dir), shared)?;

    Ok(())
}

/// Add an ignore pattern (global or per-app).
pub fn add_ignore(
    app: Option<&str>,
    pattern: &str,
    shared: &mut SharedAppConfig,
    roost_dir: &Path,
) -> Result<bool> {
    let changed = if let Some(app_name) = app {
        validate_app_name(app_name)?;
        let app_entry = shared
            .apps
            .get_mut(app_name)
            .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;
        if !app_entry.ignore.iter().any(|p| p == pattern) {
            app_entry.ignore.push(pattern.to_string());
            true
        } else {
            false
        }
    } else {
        shared.ignored.insert(pattern.to_string())
    };

    if changed {
        save_shared(&shared_config_path(roost_dir), shared)?;
        crate::gitignore::regenerate(roost_dir, &shared.ignored, &shared.apps)?;
    }

    Ok(changed)
}

/// Remove an ignore pattern (global or per-app).
pub fn remove_ignore(
    app: Option<&str>,
    pattern: &str,
    shared: &mut SharedAppConfig,
    roost_dir: &Path,
) -> Result<bool> {
    let changed = if let Some(app_name) = app {
        validate_app_name(app_name)?;
        let app_entry = shared
            .apps
            .get_mut(app_name)
            .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;
        app_entry.ignore.retain(|p| p != pattern);
        true
    } else {
        shared.ignored.remove(pattern)
    };

    if changed {
        save_shared(&shared_config_path(roost_dir), shared)?;
        crate::gitignore::regenerate(roost_dir, &shared.ignored, &shared.apps)?;
    }

    Ok(changed)
}

/// Adopt orphaned files into the active profile.
pub fn adopt_orphans(
    names: &[String],
    is_dirs: &[bool],
    shared: &mut SharedAppConfig,
    local: &LocalAppConfig,
    roost_dir: &Path,
) -> Result<usize> {
    let profile_name = local.active_profile.clone();
    validate_profile_name(&profile_name)?;

    for (name, is_dir) in names.iter().zip(is_dirs.iter()) {
        validate_app_name(name)?;
        shared.apps.insert(
            name.clone(),
            Application {
                primary_config: None,
                on_profiles: {
                    let mut s = BTreeSet::new();
                    s.insert(profile_name.clone());
                    s
                },
                is_dir: *is_dir,
                ignore: Vec::new(),
            },
        );
        if let Some(profile) = shared.profiles.get_mut(&profile_name) {
            profile.apps.insert(name.clone());
        }
    }

    save_shared(&shared_config_path(roost_dir), shared)?;

    Ok(names.len())
}
