use color_eyre::{self, eyre::eyre};
use std::{collections::HashSet, fs, path::{Path, PathBuf}};

// ── Path helpers ──

/// Compute where an entry should live inside the roost profile directory.
///
/// Checks what already exists in `profile_dir` first — this is reliable even
/// after the external symlink at `original` has been removed (e.g. during a
/// profile switch). Falls back to `original.is_dir()` only at ingest time,
/// when the original path still exists and the profile slot hasn't been
/// created yet.
///
/// Directories: `<profile_dir>/<name>`
/// Files:       `<profile_dir>/misc/<filename>`
pub fn roost_dest(profile_dir: &Path, original: &Path) -> color_eyre::Result<std::path::PathBuf> {
    let name = original
        .file_name()
        .ok_or_else(|| eyre!("no filename for {}", original.display()))?;

    let dir_candidate = profile_dir.join(name);
    let file_candidate = profile_dir.join("misc").join(name);

    if dir_candidate.exists() || dir_candidate.is_symlink() {
        return Ok(dir_candidate);
    }
    if file_candidate.exists() || file_candidate.is_symlink() {
        return Ok(file_candidate);
    }

    // Nothing in the profile dir yet — use the current type of `original`.
    if original.is_dir() {
        Ok(profile_dir.join(name))
    } else {
        Ok(profile_dir.join("misc").join(name))
    }
}

/// Check whether a path is already a symlink pointing somewhere inside `roost_dir`.
pub fn is_roost_symlink(path: &Path, roost_dir: &Path) -> bool {
    path.is_symlink()
        && fs::read_link(path)
            .map(|target| target.starts_with(roost_dir))
            .unwrap_or(false)
}

// ── Low-level operations ──

/// Create a symlink at `link` pointing to `target`.
pub fn symlink(target: &Path, link: &Path) -> color_eyre::Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link)?;

    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)?;
        } else {
            std::os::windows::fs::symlink_file(target, link)?;
        }
    }

    Ok(())
}

/// Move `from` to `to`. Tries rename first, falls back to copy + remove.
pub fn relocate(from: &Path, to: &Path) -> color_eyre::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }

    if fs::rename(from, to).is_ok() {
        return Ok(());
    }

    // Cross-device fallback
    if from.is_dir() {
        copy_dir_recursive(from, to)?;
        fs::remove_dir_all(from)?;
    } else {
        fs::copy(from, to)?;
        fs::remove_file(from)?;
    }

    Ok(())
}

fn tmp_backup_path(link_path: &Path) -> PathBuf {
    let dir = std::env::temp_dir().join("roost-backups");
    let file_name = link_path.file_name().unwrap().to_str().unwrap();
    dir.join(format!("{}.pre-roost-backup", file_name))
}

pub(crate) fn copy_dir_recursive(from: &Path, to: &Path) -> color_eyre::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

// ── High-level operations ──

/// Ingest: move an original config into roost and symlink back.
/// Used during init / adding new apps.
pub fn ingest(original: &Path, profile_dir: &Path, roost_dir: &Path) -> color_eyre::Result<()> {
    if !original.exists() {
        return Err(eyre!("source does not exist: {}", original.display()));
    }

    if is_roost_symlink(original, roost_dir) {
        return Ok(());
    }

    let dest = roost_dest(profile_dir, original)?;

    if dest.exists() {
        return Err(eyre!(
            "destination already exists: {}. Resolve manually.",
            dest.display()
        ));
    }

    relocate(original, &dest)?;

    // If the ingested directory contains a .git folder, remove it so git
    // doesn't treat it as a submodule when committing the roost repo.
    let nested_git = dest.join(".git");
    if nested_git.exists() {
        fs::remove_dir_all(&nested_git)?;
    }

    symlink(&dest, original)?;
    Ok(())
}

/// Restore: files already live in roost, just create symlinks at original locations.
/// Used when setting up an existing roost config on a new device.
///
/// `dest` is the already-resolved roost destination. We accept it as a
/// parameter instead of recomputing via `roost_dest` because the original
/// path may no longer exist on disk, which would cause `roost_dest` to
/// mis-classify it.
pub fn restore(original: &Path, dest: &Path, roost_dir: &Path) -> color_eyre::Result<()> {
    if is_roost_symlink(original, roost_dir) {
        return Ok(());
    }

    if !dest.exists() {
        return Err(eyre!("expected {} in roost but not found", dest.display()));
    }

    if original.exists() {
        return Err(eyre!(
            "{} already exists and is not a roost symlink. Resolve manually.",
            original.display()
        ));
    }

    symlink(dest, original)?;
    Ok(())
}

/// Ensure every app in the config has a working symlink at its link_path.
/// Also sets up intermediate source symlinks for any sourced apps.
/// Called after init and sync to pick up apps pulled from a remote.
/// Apps absent from `local.link_paths` are silently skipped (platform-only apps).
pub fn ensure_links(
    config: &crate::app::SharedAppConfig,
    local: &crate::app::LocalAppConfig,
    roost_dir: &Path,
) {
    // Pass 1: ensure intermediate (source) symlinks for sourced apps.
    for (prof_name, profile) in &config.profiles {
        for (app_name, source_profile) in &profile.app_sources {
            let Some(link_path) = local.link_paths.get(app_name) else {
                continue;
            };
            let prof_dir = roost_dir.join(prof_name);
            let source_prof_dir = roost_dir.join(source_profile);

            let dest = match roost_dest(&prof_dir, link_path) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let source_path = match roost_dest(&source_prof_dir, link_path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            if dest.is_symlink() {
                let current = fs::read_link(&dest).unwrap_or_default();
                if current == source_path {
                    continue; // Already correct
                }
                if fs::remove_file(&dest).is_err() {
                    continue;
                }
            } else if dest.exists() {
                continue; // Real files present — don't clobber
            }

            if let Err(e) = symlink(&source_path, &dest) {
                eprintln!(
                    "  warn: could not symlink {} → {}: {}",
                    dest.display(),
                    source_path.display(),
                    e
                );
            }
        }
    }

    // Pass 2: ensure external symlinks (link_path → roost slot) for active
    // profile apps only.
    let active = &local.active_profile;
    for (app_name, app) in &config.apps {
        if !app.on_profiles.contains(active) {
            continue;
        }
        let Some(link_path) = local.link_paths.get(app_name) else {
            continue;
        };
        if is_roost_symlink(link_path, roost_dir) {
            continue;
        }
        let prof_dir = roost_dir.join(active);
        let dest = match roost_dest(&prof_dir, link_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if !dest.exists() {
            continue;
        }
        if link_path.exists() {
            let backup = tmp_backup_path(link_path);
            if relocate(link_path, &backup).is_err() {
                continue;
            }
        }
        let _ = restore(link_path, &dest, roost_dir);
    }
}

/// Switch symlinks from one profile to another.
///
/// 1. Remove all roost symlinks belonging to the old profile
/// 2. Set up intermediate source symlinks for sourced apps in the new profile
/// 3. Create external symlinks for apps in the new profile
pub fn switch_links(
    old_profile: &str,
    new_profile: &str,
    config: &crate::app::SharedAppConfig,
    local: &crate::app::LocalAppConfig,
    roost_dir: &Path,
) {
    // Remove symlinks for old profile's apps
    if let Some(old_prof) = config.profiles.get(old_profile) {
        for app_name in &old_prof.apps {
            let Some(link_path) = local.link_paths.get(app_name) else {
                continue;
            };
            if is_roost_symlink(link_path, roost_dir)
                && let Err(e) = fs::remove_file(link_path) {
                    eprintln!(
                        "  warn: could not remove symlink {}: {}",
                        link_path.display(),
                        e
                    );
                }
        }
    }

    let Some(new_prof) = config.profiles.get(new_profile) else {
        return;
    };
    let new_prof_dir = roost_dir.join(new_profile);

    // Snapshot to avoid borrow-across-method-call issues
    let app_names: Vec<String> = new_prof.apps.iter().cloned().collect();
    let app_sources = new_prof.app_sources.clone();

    for app_name in &app_names {
        let Some(link_path) = local.link_paths.get(app_name) else {
            continue; // Not on this device
        };
        // Ensure intermediate symlink for sourced apps
        if let Some(source_profile) = app_sources.get(app_name) {
            let source_prof_dir = roost_dir.join(source_profile);
            if let (Ok(dest), Ok(source_path)) = (
                roost_dest(&new_prof_dir, link_path),
                roost_dest(&source_prof_dir, link_path),
            )
                && !dest.exists() && !dest.is_symlink()
                    && symlink(&source_path, &dest).is_err() {
                        continue;
                    }
        }

        // Create external symlink
        if is_roost_symlink(link_path, roost_dir) {
            continue;
        }
        let dest = match roost_dest(&new_prof_dir, link_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if !dest.exists() {
            continue;
        }
        if link_path.exists() {
            let backup = tmp_backup_path(link_path);
            if relocate(link_path, &backup).is_err() {
                continue;
            }
        }
        if let Err(e) = symlink(&dest, link_path) {
            eprintln!(
                "  warn: could not symlink {} → {}: {}",
                link_path.display(),
                dest.display(),
                e
            );
        }
    }
}

/// For apps in the given profile that have no entry in `local.link_paths`,
/// try to find them on the filesystem by scanning known source directories.
/// Populates `link_paths` for any matches found. Returns the number resolved.
pub fn resolve_missing_link_paths(
    profile_name: &str,
    config: &crate::app::SharedAppConfig,
    local: &mut crate::app::LocalAppConfig,
) -> usize {
    let profile = match config.profiles.get(profile_name) {
        Some(p) => p,
        None => return 0,
    };

    let sources = crate::scanner::get_likely_sources();
    let mut resolved = 0;

    for app_name in &profile.apps {
        if local.link_paths.contains_key(app_name) {
            continue;
        }

        if let Some(path) = find_app_on_filesystem(app_name, &sources) {
            local.link_paths.insert(app_name.clone(), path);
            resolved += 1;
        }
    }

    resolved
}

/// Search known source directories for an entry matching `app_name`.
/// Checks for both a directory and a file with the exact name.
/// Returns None if not found — the app simply won't be linked on this device.
pub fn find_app_on_filesystem(
    app_name: &str,
    sources: &[PathBuf],
) -> Option<PathBuf> {
    for source in sources {
        let dir_candidate = source.join(app_name);
        if dir_candidate.is_dir() {
            return Some(dir_candidate);
        }
        let file_candidate = source.join(app_name);
        if file_candidate.is_file() {
            return Some(file_candidate);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let dot_candidate = home.join(format!(".{}", app_name));
        if dot_candidate.exists() {
            return Some(dot_candidate);
        }
    }

    None
}

/// Scan all profiles for apps referenced in `profile.apps` but missing from
/// `config.apps`. Create minimal Application entries for any found.
/// Returns the number of entries created.
pub fn adopt_orphaned_apps(
    config: &mut crate::app::SharedAppConfig,
    config_path: &Path,
) -> usize {
    let mut adopted = 0;

    for (prof_name, profile) in &config.profiles {
        for app_name in &profile.apps {
            if config.apps.contains_key(app_name) {
                continue;
            }

            config.apps.insert(
                app_name.clone(),
                crate::app::Application {
                    name: app_name.clone(),
                    primary_config: None,
                    on_profiles: vec![prof_name.clone()],
                },
            );
            adopted += 1;
        }
    }

    if adopted > 0 {
        for app in config.apps.values_mut() {
            app.on_profiles.sort();
            app.on_profiles.dedup();
        }
        let _ = config.save(config_path);
    }

    adopted
}

/// Unlink: remove symlink and restore files to their original location.
///
/// For sourced apps (where the roost slot is itself a symlink), both the
/// external and intermediate symlinks are removed and the source files are
/// left untouched — this profile never owned them.
pub fn unlink(original: &Path, profile_dir: &Path, roost_dir: &Path) -> color_eyre::Result<()> {
    if !is_roost_symlink(original, roost_dir) {
        return Err(eyre!("{} is not a roost symlink", original.display()));
    }

    let dest = roost_dest(profile_dir, original)?;

    if dest.is_symlink() {
        // Sourced app — remove both symlinks, leave source files alone
        fs::remove_file(original)?;
        fs::remove_file(&dest)?;
        return Ok(());
    }

    // Owned app — remove symlink, move real files back
    fs::remove_file(original)?;
    relocate(&dest, original)?;
    Ok(())
}

// ── Source linking ──

/// Check whether setting `(target_profile, app_name) → source_profile`
/// would create a cycle by following the chain of `app_sources` entries.
pub fn detect_source_cycle(
    target_profile: &str,
    app_name: &str,
    proposed_source: &str,
    config: &crate::app::SharedAppConfig,
) -> bool {
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(target_profile.to_string());

    let mut current = proposed_source.to_string();

    loop {
        if visited.contains(&current) {
            return true; // Cycle detected
        }
        visited.insert(current.clone());

        let Some(prof) = config.profiles.get(&current) else {
            return false; // Profile doesn't exist — chain ends
        };
        let Some(next) = prof.app_sources.get(app_name) else {
            return false; // No further redirect — clean termination
        };
        current = next.clone();
    }
}

/// Set an app's source in a profile.
///
/// Records `source_profile` in `roost.toml` and creates the intermediate
/// symlink `~/.roost/<profile>/<app> → ~/.roost/<source_profile>/<app>`.
///
/// Enforces two invariants:
/// - The source profile must own real files for this app (not itself be sourced).
/// - No cycles are created.
#[allow(dead_code)]
pub fn set_app_source(
    profile_name: &str,
    app_name: &str,
    source_profile: &str,
    config: &mut crate::app::SharedAppConfig,
    config_path: &Path,
    roost_dir: &Path,
    link_paths: &std::collections::HashMap<String, std::path::PathBuf>,
) -> color_eyre::Result<()> {
    if profile_name == source_profile {
        return Err(eyre!("A profile cannot source an app from itself."));
    }

    // Source profile must exist
    let source_prof = config
        .profiles
        .get(source_profile)
        .ok_or_else(|| eyre!("Source profile '{}' does not exist.", source_profile))?;

    // Source must own real files — it cannot itself be sourced
    if source_prof.app_sources.contains_key(app_name) {
        return Err(eyre!(
            "'{}' in profile '{}' is itself sourced — use the original source instead.",
            app_name,
            source_profile
        ));
    }

    // App must be managed by the source profile
    if !source_prof.apps.contains(app_name) {
        return Err(eyre!(
            "App '{}' is not in profile '{}'.",
            app_name,
            source_profile
        ));
    }

    let link_path = link_paths
        .get(app_name)
        .ok_or_else(|| eyre!("No local link path for app '{}' on this device.", app_name))?;

    let target_prof_dir = roost_dir.join(profile_name);
    let source_prof_dir = roost_dir.join(source_profile);

    let dest = roost_dest(&target_prof_dir, link_path)?;
    let source_path = roost_dest(&source_prof_dir, link_path)?;

    // Replace an existing intermediate symlink; refuse to clobber real files
    if dest.is_symlink() {
        fs::remove_file(&dest)?;
    } else if dest.exists() {
        return Err(eyre!(
            "{} contains real files. Remove the app first, then set its source.",
            dest.display()
        ));
    }

    symlink(&source_path, &dest)?;

    let profile = config
        .profiles
        .get_mut(profile_name)
        .ok_or_else(|| eyre!("Profile '{}' not found.", profile_name))?;
    profile
        .app_sources
        .insert(app_name.to_string(), source_profile.to_string());

    config.save(config_path)?;
    Ok(())
}

/// Remove an app's source override, deleting the intermediate symlink and
/// clearing the entry from `roost.toml`.
#[allow(dead_code)]
pub fn clear_app_source(
    profile_name: &str,
    app_name: &str,
    config: &mut crate::app::SharedAppConfig,
    config_path: &Path,
    roost_dir: &Path,
    link_paths: &std::collections::HashMap<String, std::path::PathBuf>,
) -> color_eyre::Result<()> {
    let link_path = link_paths
        .get(app_name)
        .ok_or_else(|| eyre!("No local link path for app '{}' on this device.", app_name))?;

    let prof_dir = roost_dir.join(profile_name);
    let dest = roost_dest(&prof_dir, link_path)?;

    if dest.is_symlink() {
        fs::remove_file(&dest)?;
    }

    let profile = config
        .profiles
        .get_mut(profile_name)
        .ok_or_else(|| eyre!("Profile '{}' not found.", profile_name))?;
    profile.app_sources.remove(app_name);

    config.save(config_path)?;
    Ok(())
}

/// Import an app from `source_profile` into `to_profile` via symlink chain.
///
/// Unlike `set_app_source` (which assumes the app already exists in the target
/// profile), this is used when the target profile doesn't have the app at all.
/// It registers the app in the target profile, creates the intermediate symlink
/// `~/.roost/<to>/<app> → ~/.roost/<source>/<app>`, and updates the external
/// symlink at `link_path` to point through the new chain (since we're currently
/// on `to_profile`).
pub fn import_app_from_profile(
    app_name: &str,
    to_profile: &str,
    source_profile: &str,
    config: &mut crate::app::SharedAppConfig,
    config_path: &Path,
    roost_dir: &Path,
    local: &mut crate::app::LocalAppConfig,
) -> color_eyre::Result<()> {
    if to_profile == source_profile {
        return Err(eyre!("Cannot import from the same profile."));
    }

    // Source must exist and own real files for this app
    let source_prof = config
        .profiles
        .get(source_profile)
        .ok_or_else(|| eyre!("Source profile '{}' not found.", source_profile))?;

    if source_prof.app_sources.contains_key(app_name) {
        return Err(eyre!(
            "'{}' in '{}' is itself sourced — use the original instead.",
            app_name,
            source_profile
        ));
    }

    if !source_prof.apps.contains(app_name) {
        return Err(eyre!(
            "App '{}' is not in profile '{}'.",
            app_name,
            source_profile
        ));
    }

    // Target must not already have this app
    let to_prof = config
        .profiles
        .get(to_profile)
        .ok_or_else(|| eyre!("Profile '{}' not found.", to_profile))?;

    if to_prof.apps.contains(app_name) {
        return Err(eyre!(
            "App '{}' already exists in profile '{}'.",
            app_name,
            to_profile
        ));
    }

    if detect_source_cycle(to_profile, app_name, source_profile, config) {
        return Err(eyre!("Would create a symlink cycle."));
    }

    let link_path = if let Some(lp) = local.link_paths.get(app_name) {
        lp.clone()
    } else if let Some(detected) = find_app_on_filesystem(app_name, &crate::scanner::get_likely_sources()) {
        local.link_paths.insert(app_name.to_string(), detected.clone());
        detected
    } else {
        return Err(eyre!(
            "No link path for '{}' on this device. Place its config at a standard location (e.g. ~/.config/{}) or run `roost add <path>` first.",
            app_name, app_name
        ));
    };

    let to_prof_dir = roost_dir.join(to_profile);
    let source_prof_dir = roost_dir.join(source_profile);

    let dest = roost_dest(&to_prof_dir, &link_path)?;
    let source_path = roost_dest(&source_prof_dir, &link_path)?;

    // Create intermediate symlink: ~/.roost/<to>/<app> → ~/.roost/<source>/<app>
    if dest.is_symlink() {
        fs::remove_file(&dest)?;
    } else if dest.exists() {
        return Err(eyre!(
            "{} already has real files — resolve manually first.",
            dest.display()
        ));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    symlink(&source_path, &dest)?;

    // Update the external symlink (link_path) to point through the new chain
    if link_path.is_symlink() {
        fs::remove_file(&link_path)?;
    } else if link_path.exists() {
        let backup = tmp_backup_path(&link_path);
        relocate(&link_path, &backup)?;
    }
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent)?;
    }
    symlink(&dest, &link_path)?;

    // Register in config
    {
        let to_prof = config
            .profiles
            .get_mut(to_profile)
            .ok_or_else(|| eyre!("Profile '{}' not found.", to_profile))?;
        to_prof.apps.insert(app_name.to_string());
        to_prof
            .app_sources
            .insert(app_name.to_string(), source_profile.to_string());
    }
    if let Some(app) = config.apps.get_mut(app_name) {
        if !app.on_profiles.contains(&to_profile.to_string()) {
            app.on_profiles.push(to_profile.to_string());
        }
    } else {
        config.apps.insert(
            app_name.to_string(),
            crate::app::Application {
                name: app_name.to_string(),
                primary_config: None,
                on_profiles: vec![to_profile.to_string()],
            },
        );
    }

    config.save(config_path)?;
    Ok(())
}

/// Copy an app's files from one profile to another so both profiles own
/// independent copies. No symlink relationship is set up.
pub fn copy_to_profile(
    app_name: &str,
    from_profile: &str,
    to_profile: &str,
    config: &mut crate::app::SharedAppConfig,
    config_path: &Path,
    roost_dir: &Path,
    link_paths: &std::collections::HashMap<String, std::path::PathBuf>,
) -> color_eyre::Result<()> {
    if from_profile == to_profile {
        return Err(eyre!("Source and destination profile are the same."));
    }

    let link_path = link_paths
        .get(app_name)
        .ok_or_else(|| eyre!("No local link path for app '{}' on this device.", app_name))?;

    let from_prof_dir = roost_dir.join(from_profile);
    let to_prof_dir = roost_dir.join(to_profile);

    let from_dest = roost_dest(&from_prof_dir, link_path)?;
    let to_dest = roost_dest(&to_prof_dir, link_path)?;

    if to_dest.exists() || to_dest.is_symlink() {
        return Err(eyre!(
            "'{}' already exists in profile '{}'. Resolve manually.",
            app_name,
            to_profile
        ));
    }

    if !to_prof_dir.exists() {
        fs::create_dir_all(&to_prof_dir)?;
    }

    if !from_dest.exists() && !from_dest.is_symlink() {
        return Err(eyre!(
            "App '{}' has no files in profile '{}' (expected at {}).",
            app_name,
            from_profile,
            from_dest.display()
        ));
    }

    // Copy files — both profiles own independent copies
    if from_dest.is_dir() {
        copy_dir_recursive(&from_dest, &to_dest)?;
    } else {
        if let Some(parent) = to_dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&from_dest, &to_dest)?;
    }

    // Add app to target profile in config
    if let Some(to_prof) = config.profiles.get_mut(to_profile) {
        to_prof.apps.insert(app_name.to_string());
    }
    if let Some(app) = config.apps.get_mut(app_name)
        && !app.on_profiles.contains(&to_profile.to_string()) {
            app.on_profiles.push(to_profile.to_string());
        }

    config.save(config_path)?;
    Ok(())
}

#[cfg(test)]
mod tests;
