use color_eyre::{Result, eyre::bail};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::app::{LocalAppConfig, SharedAppConfig, profile_dir};

pub const MISC_DIR_NAME: &str = "misc";
const BACKUP_DIR_NAME: &str = ".backups";

/// Reject paths that escape the home directory or contain parent-dir components.
/// This prevents a malicious synced roost.toml from causing sensitive file
/// overwrites via ingest/restore operations.
pub fn validate_path_in_home(path: &Path, home: &Path) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());

    let under_home = canonical.starts_with(&canonical_home);

    // Allow paths under the system temp directory (for integration tests using tempfile)
    let temp_dir = std::env::temp_dir();
    let canonical_temp = temp_dir.canonicalize().unwrap_or_else(|_| temp_dir.to_path_buf());
    let under_temp = canonical.starts_with(&canonical_temp)
        || path.starts_with(&temp_dir);

    if !under_home && !under_temp {
        bail!(
            "Path '{}' is outside the home directory ({})",
            path.display(),
            home.display()
        );
    }

    // Reject explicit parent-dir references (defense in depth)
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            bail!(
                "Path '{}' contains parent directory references (..)",
                path.display()
            );
        }
    }

    Ok(())
}

// move origin into roost, symlink origin back to roost
pub fn ingest(origin: &Path, roost_dir: &Path, app_name: &str, is_dir: bool) -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    validate_path_in_home(origin, &home)?;
    crate::app::validate_app_name(app_name)?;

    if !origin.exists() {
        bail!("origin path does not exist: {}", origin.display());
    }

    if origin.is_symlink() {
        bail!("origin already a symlink: {}", origin.display());
    }

    let backup_dest = roost_dir.join(BACKUP_DIR_NAME).join(app_name);
    fs::create_dir_all(roost_dir.join(BACKUP_DIR_NAME))?;

    if !is_dir {
        fs::create_dir_all(roost_dir.join(MISC_DIR_NAME))?;
    }
    let app_dest = app_dest(roost_dir, app_name, is_dir);

    // backup before moving
    if origin.is_dir() {
        copy_dir_recursive(origin, &backup_dest)?;
    } else {
        fs::copy(origin, &backup_dest)?;
    }

    // move origin -> roost dest
    fs::rename(origin, &app_dest)?;

    // symlink: origin -> roost dest
    create_symlink(&app_dest, origin, is_dir)?;

    Ok(())
}

// resolve where an app lives within the roost directory
pub fn app_dest(roost_dir: &Path, app_name: &str, is_dir: bool) -> PathBuf {
    if is_dir {
        roost_dir.join(app_name)
    } else {
        roost_dir.join(MISC_DIR_NAME).join(app_name)
    }
}

// create symlink at origin pointing to roost (for fresh setup from git pull)
pub fn restore(origin: &Path, roost_dir: &Path, app_name: &str, is_dir: bool) -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    validate_path_in_home(origin, &home)?;
    crate::app::validate_app_name(app_name)?;

    let dest = app_dest(roost_dir, app_name, is_dir);

    if !dest.exists() {
        bail!("app files not found in roost: {}", dest.display());
    }

    if origin.is_symlink() {
        let target = fs::read_link(origin)?;
        if target == dest {
            return Ok(());
        }
        bail!(
            "origin exists as symlink to wrong target: {} -> {}",
            origin.display(),
            target.display()
        );
    }

    if origin.exists() {
        bail!(
            "origin exists as real file/dir (use ingest instead): {}",
            origin.display()
        );
    }

    if let Some(parent) = origin.parent() {
        fs::create_dir_all(parent)?;
    }

    create_symlink(&dest, origin, is_dir)?;
    Ok(())
}

// reverse of ingest: remove symlink, move files back to origin
pub fn unlink(origin: &Path, roost_dir: &Path, app_name: &str, is_dir: bool) -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    validate_path_in_home(origin, &home)?;
    crate::app::validate_app_name(app_name)?;

    let dest = app_dest(roost_dir, app_name, is_dir);

    if !origin.is_symlink() {
        bail!("origin is not a symlink: {}", origin.display());
    }

    let target = fs::read_link(origin)?;
    if target != dest {
        bail!(
            "symlink points to unexpected target: {} -> {} (expected {})",
            origin.display(),
            target.display(),
            dest.display()
        );
    }

    fs::remove_file(origin)?;

    // move roost files back
    if dest.exists() {
        fs::rename(&dest, origin)?;
    }

    // clean up empty misc/ dir
    if !is_dir {
        let misc = roost_dir.join(MISC_DIR_NAME);
        if misc.exists() && fs::read_dir(&misc)?.next().is_none() {
            let _ = fs::remove_dir(&misc);
        }
    }

    Ok(())
}

// verify all configured symlinks exist, create missing ones, back up conflicts
pub fn ensure_links(
    config: &SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<Vec<String>> {
    let mut actions = Vec::new();
    let profile_name = &local.active_profile;
    crate::app::validate_profile_name(profile_name)?;
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("active profile '{}' not found", profile_name))?;

    let pdir = profile_dir(roost_dir, profile_name);
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    for app_name in &profile.apps {
        let app = match config.apps.get(app_name) {
            Some(a) => a,
            None => {
                actions.push(format!("SKIP: app '{}' not found in config", app_name));
                continue;
            }
        };

        let dest = app_dest(&pdir, app_name, app.is_dir);

        // If link_path is missing, try to auto-discover from existing symlinks
        let origin = if let Some(p) = local.link_paths.get(app_name) {
            if let Err(e) = validate_path_in_home(p, &home) {
                actions.push(format!(
                    "SKIP: app '{}' has invalid link_path ({}): {}",
                    app_name,
                    p.display(),
                    e
                ));
                continue;
            }
            p.clone()
        } else {
            let candidates = [
                home.join(".config").join(app_name),
                home.join(format!(".{}", app_name)),
                home.join(app_name),
            ];
            let mut discovered: Option<PathBuf> = None;
            for candidate in &candidates {
                if candidate.is_symlink() {
                    if let Ok(target) = fs::read_link(candidate) {
                        if target == dest {
                            discovered = Some(candidate.clone());
                            break;
                        }
                    }
                }
            }
            match discovered {
                Some(p) => {
                    actions.push(format!(
                        "AUTO-DISCOVERED link_path: {} -> {}",
                        app_name,
                        p.display()
                    ));
                    local.link_paths.insert(app_name.clone(), p.clone());
                    p
                }
                None => {
                    actions.push(format!("SKIP: no link_path for '{}'", app_name));
                    continue;
                }
            }
        };

        // handle whatever is currently at origin
        if origin.is_symlink() {
            let target = fs::read_link(&origin)?;
            if target == dest {
                continue; // already correct
            }
            // symlink points elsewhere, back it up
            let backup = roost_dir
                .join(BACKUP_DIR_NAME)
                .join(format!("conflict-{}", app_name));
            fs::create_dir_all(roost_dir.join(BACKUP_DIR_NAME))?;
            fs::rename(&origin, &backup)?;
            actions.push(format!(
                "BACKED UP conflicting symlink: {} -> {}",
                origin.display(),
                backup.display()
            ));
        } else if origin.exists() {
            // real file/dir at origin, back it up
            let backup = roost_dir
                .join(BACKUP_DIR_NAME)
                .join(format!("conflict-{}", app_name));
            fs::create_dir_all(roost_dir.join(BACKUP_DIR_NAME))?;
            copy_item(&origin, &backup)?;
            remove_item(&origin)?;
            actions.push(format!(
                "BACKED UP conflicting path: {} -> {}",
                origin.display(),
                backup.display()
            ));
        }

        if let Some(parent) = origin.parent() {
            fs::create_dir_all(parent)?;
        }
        create_symlink(&dest, &origin, app.is_dir)?;
        actions.push(format!(
            "LINKED: {} -> {}",
            origin.display(),
            dest.display()
        ));
    }

    Ok(actions)
}

// remove old profile symlinks, create new profile symlinks
pub fn switch_profile(
    old_profile: &str,
    new_profile: &str,
    config: &SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<()> {
    crate::app::validate_profile_name(old_profile)?;
    crate::app::validate_profile_name(new_profile)?;

    if !config.profiles.contains_key(new_profile) {
        bail!("target profile '{}' does not exist", new_profile);
    }

    // remove symlinks for old profile
    let old_dir = profile_dir(roost_dir, old_profile);
    if let Some(old) = config.profiles.get(old_profile) {
        for app_name in &old.apps {
            let app = match config.apps.get(app_name) {
                Some(a) => a,
                None => continue,
            };
            let origin = match local.link_paths.get(app_name) {
                Some(p) => p.clone(),
                None => continue,
            };
            let dest = app_dest(&old_dir, app_name, app.is_dir);

            if origin.is_symlink()
                && let Ok(target) = fs::read_link(&origin)
                && target == dest
            {
                let _ = fs::remove_file(&origin);
            }
        }
    }

    // create symlinks for new profile
    let new_dir = profile_dir(roost_dir, new_profile);
    if let Some(new) = config.profiles.get(new_profile) {
        for app_name in &new.apps {
            let app = match config.apps.get(app_name) {
                Some(a) => a,
                None => continue,
            };
            let origin = match local.link_paths.get(app_name) {
                Some(p) => p.clone(),
                None => continue,
            };
            let dest = app_dest(&new_dir, app_name, app.is_dir);

            if dest.exists() {
                if let Some(parent) = origin.parent() {
                    fs::create_dir_all(parent)?;
                }
                // Handle existing origin (residual symlink/file from prior state)
                if origin.is_symlink() {
                    if let Ok(target) = fs::read_link(&origin) {
                        if target == dest {
                            continue;
                        }
                    }
                    let backup = roost_dir
                        .join(BACKUP_DIR_NAME)
                        .join(format!("conflict-{}", app_name));
                    fs::create_dir_all(roost_dir.join(BACKUP_DIR_NAME))?;
                    if backup.exists() {
                        remove_item(&backup)?;
                    }
                    fs::rename(&origin, &backup)?;
                } else if origin.exists() {
                    let backup = roost_dir
                        .join(BACKUP_DIR_NAME)
                        .join(format!("conflict-{}", app_name));
                    if backup.exists() {
                        remove_item(&backup)?;
                    }
                    copy_item(&origin, &backup)?;
                    remove_item(&origin)?;
                }
                create_symlink(&dest, &origin, app.is_dir)?;
            }
        }
    }

    local.active_profile = new_profile.to_string();
    Ok(())
}

// cross-profile symlink chain (zero-copy): target -> source
pub fn import_from(
    app_name: &str,
    source_profile: &str,
    target_profile: &str,
    roost_dir: &Path,
) -> Result<()> {
    let source_dir = roost_dir.join(source_profile).join(app_name);
    let target_dir = roost_dir.join(target_profile).join(app_name);

    if !source_dir.exists() {
        bail!("source path does not exist: {}", source_dir.display());
    }
    if target_dir.exists() {
        bail!("target already exists: {}", target_dir.display());
    }
    if source_dir.is_symlink() {
        bail!(
            "{} in profile {} is a symlink (must import from source file)",
            source_dir.display(),
            source_profile
        );
    }

    create_symlink(&source_dir, &target_dir, true)?;
    Ok(())
}

// independent copy of app files into another profile
pub fn copy_to(
    app_name: &str,
    source_profile: &str,
    target_profile: &str,
    roost_dir: &Path,
) -> Result<()> {
    let source_dir = roost_dir.join(source_profile).join(app_name);
    let target_dir = roost_dir.join(target_profile).join(app_name);

    if !source_dir.exists() {
        bail!("source path does not exist: {}", source_dir.display());
    }
    if target_dir.exists() {
        bail!("target already exists: {}", target_dir.display());
    }

    fs::create_dir_all(target_dir.parent().unwrap())?;
    copy_dir_recursive(&source_dir, &target_dir)?;
    Ok(())
}

/// Copy a single filesystem item, preserving its type.
/// Files are copied, directories are recursively copied, symlinks are recreated.
pub fn copy_item(src: &Path, dst: &Path) -> Result<()> {
    if src.is_symlink() {
        let target = fs::read_link(src)?;
        create_symlink(&target, dst, src.is_dir())?;
    } else if src.is_dir() {
        copy_dir_recursive(src, dst)?;
    } else {
        fs::copy(src, dst)?;
    }
    Ok(())
}

// recursive directory copy — preserves symlinks as symlinks
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_symlink() {
            let target = fs::read_link(&src_path)?;
            create_symlink(&target, &dst_path, src_path.is_dir())?;
        } else if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Remove a filesystem item using the correct method for its type.
pub fn remove_item(path: &Path) -> Result<()> {
    if path.is_symlink() || path.is_file() {
        fs::remove_file(path)?;
    } else if path.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub enum LinkStatus {
    Ok {
        app: String,
        origin: PathBuf,
        target: PathBuf,
    },
    Missing {
        app: String,
        origin: PathBuf,
        target: PathBuf,
    },
    Broken {
        app: String,
        origin: PathBuf,
        actual: PathBuf,
        expected: PathBuf,
    },
    Conflict {
        app: String,
        origin: PathBuf,
    },
    NoLinkPath {
        app: String,
    },
}

pub fn check_links(
    config: &SharedAppConfig,
    local: &LocalAppConfig,
    roost_dir: &Path,
) -> Result<Vec<LinkStatus>> {
    let profile_name = &local.active_profile;
    let profile = match config.profiles.get(profile_name) {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };
    let pdir = crate::app::profile_dir(roost_dir, profile_name);

    let mut results = Vec::new();
    for app_name in &profile.apps {
        let app_entry = match config.apps.get(app_name) {
            Some(a) => a,
            None => continue,
        };
        let origin = match local.link_paths.get(app_name) {
            Some(p) => p.clone(),
            None => {
                results.push(LinkStatus::NoLinkPath {
                    app: app_name.clone(),
                });
                continue;
            }
        };
        let dest = app_dest(&pdir, app_name, app_entry.is_dir);

        if !origin.exists() {
            results.push(LinkStatus::Missing {
                app: app_name.clone(),
                origin,
                target: dest,
            });
        } else if origin.is_symlink() {
            match fs::read_link(&origin) {
                Ok(target) if target == dest => {
                    results.push(LinkStatus::Ok {
                        app: app_name.clone(),
                        origin,
                        target: dest,
                    });
                }
                Ok(target) => {
                    results.push(LinkStatus::Broken {
                        app: app_name.clone(),
                        origin: origin.clone(),
                        actual: target,
                        expected: dest,
                    });
                }
                Err(_) => {
                    results.push(LinkStatus::Broken {
                        app: app_name.clone(),
                        origin: origin.clone(),
                        actual: PathBuf::new(),
                        expected: dest,
                    });
                }
            }
        } else {
            results.push(LinkStatus::Conflict {
                app: app_name.clone(),
                origin,
            });
        }
    }
    Ok(results)
}

#[derive(Debug, Clone)]
pub struct Orphan {
    pub profile: String,
    pub name: String,
    pub is_dir: bool,
    pub path: PathBuf,
}

pub fn find_orphans(config: &SharedAppConfig, roost_dir: &Path) -> Result<Vec<Orphan>> {
    let mut orphans = Vec::new();
    for prof_name in config.profiles.keys() {
        let pdir = crate::app::profile_dir(roost_dir, prof_name);
        if !pdir.exists() {
            continue;
        }
        for entry in (fs::read_dir(&pdir)?).flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == MISC_DIR_NAME || name.starts_with('.') {
                continue;
            }
            if !config.apps.contains_key(&name) {
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                orphans.push(Orphan {
                    profile: prof_name.clone(),
                    name,
                    is_dir,
                    path: entry.path(),
                });
            }
        }
        let misc_dir = pdir.join(MISC_DIR_NAME);
        if misc_dir.exists() {
            for entry in (fs::read_dir(&misc_dir)?).flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !config.apps.contains_key(&name) {
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    orphans.push(Orphan {
                        profile: prof_name.clone(),
                        name,
                        is_dir,
                        path: entry.path(),
                    });
                }
            }
        }
    }
    Ok(orphans)
}

// cross-platform symlink creation
#[cfg(unix)]
pub fn create_symlink(target: &Path, link: &Path, _is_dir: bool) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
pub fn create_symlink(target: &Path, link: &Path, is_dir: bool) -> io::Result<()> {
    if is_dir {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

#[cfg(test)]
mod tests;
