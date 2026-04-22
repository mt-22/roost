use color_eyre::{Result, eyre::bail};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::app::{LocalAppConfig, SharedAppConfig};

const MISC_DIR_NAME: &str = "misc";
const BACKUP_DIR_NAME: &str = ".backups";

pub fn ingest(origin: &Path, roost_dir: &Path, app_name: &str, is_dir: bool) -> Result<()> {
    if !origin.exists() {
        bail!("origin path does not exist: {}", origin.display());
    }
    let meta = fs::symlink_metadata(origin)?;

    if meta.is_symlink() {
        bail!("origin already a symlink: {}", origin.display());
    }
    let backup_dest = roost_dir.join(BACKUP_DIR_NAME).join(app_name);

    fs::create_dir_all(roost_dir.join(BACKUP_DIR_NAME))?;

    let app_dest: PathBuf;
    if is_dir {
        app_dest = roost_dir.join(app_name);
    } else {
        fs::create_dir_all(roost_dir.join(MISC_DIR_NAME))?;
        app_dest = roost_dir.join(MISC_DIR_NAME).join(app_name);
    }

    if origin.is_dir() {
        copy_dir_recursive(&origin, &backup_dest)?;
    } else {
        fs::copy(&origin, &backup_dest)?;
    }

    fs::rename(&origin, &app_dest)?;

    create_symlink(&app_dest, &origin, is_dir)?;

    Ok(())
}

pub fn app_dest(roost_dir: &Path, app_name: &str, is_dir: bool) -> PathBuf {
    if is_dir {
        roost_dir.join(app_name)
    } else {
        roost_dir.join(MISC_DIR_NAME).join(app_name)
    }
}

pub fn restore(
    origin: &Path,
    roost_dir: &Path,
    app_name: &str,
    is_dir: bool,
) -> Result<()> {
    let dest = app_dest(roost_dir, app_name, is_dir);

    if !dest.exists() {
        bail!("app files not found in roost: {}", dest.display());
    }

    if let Ok(meta) = fs::symlink_metadata(origin) {
        if meta.is_symlink() {
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

pub fn unlink(
    origin: &Path,
    roost_dir: &Path,
    app_name: &str,
    is_dir: bool,
) -> Result<()> {
    let dest = app_dest(roost_dir, app_name, is_dir);

    let meta = fs::symlink_metadata(origin)?;
    if !meta.is_symlink() {
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

    if dest.exists() {
        fs::rename(&dest, origin)?;
    }

    if !is_dir {
        let misc = roost_dir.join(MISC_DIR_NAME);
        if misc.exists() && fs::read_dir(&misc)?.next().is_none() {
            let _ = fs::remove_dir(&misc);
        }
    }

    Ok(())
}

pub fn ensure_links(
    config: &SharedAppConfig,
    local: &LocalAppConfig,
    roost_dir: &Path,
) -> Result<Vec<String>> {
    let mut actions = Vec::new();
    let profile_name = &local.active_profile;
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("active profile '{}' not found", profile_name))?;

    for app_name in &profile.apps {
        let app = match config.apps.get(app_name) {
            Some(a) => a,
            None => {
                actions.push(format!("SKIP: app '{}' not found in config", app_name));
                continue;
            }
        };

        let origin = match local.link_paths.get(app_name) {
            Some(p) => p,
            None => {
                actions.push(format!("SKIP: no link_path for '{}'", app_name));
                continue;
            }
        };

        let profile_dir = roost_dir.join(profile_name);
        let dest = app_dest(&profile_dir, app_name, app.is_dir);

        match fs::symlink_metadata(origin) {
            Ok(meta) if meta.is_symlink() => {
                let target = fs::read_link(origin)?;
                if target == dest {
                    continue;
                }
                let backup = roost_dir
                    .join(BACKUP_DIR_NAME)
                    .join(format!("conflict-{}", app_name));
                fs::create_dir_all(roost_dir.join(BACKUP_DIR_NAME))?;
                fs::rename(origin, &backup)?;
                actions.push(format!(
                    "BACKED UP conflicting symlink: {} -> {}",
                    origin.display(),
                    backup.display()
                ));
            }
            Ok(_) => {
                let backup = roost_dir
                    .join(BACKUP_DIR_NAME)
                    .join(format!("conflict-{}", app_name));
                fs::create_dir_all(roost_dir.join(BACKUP_DIR_NAME))?;
                copy_dir_recursive(origin, &backup)?;
                fs::remove_dir_all(origin)?;
                actions.push(format!(
                    "BACKED UP conflicting path: {} -> {}",
                    origin.display(),
                    backup.display()
                ));
            }
            Err(_) => {}
        }

        if let Some(parent) = origin.parent() {
            fs::create_dir_all(parent)?;
        }
        create_symlink(&dest, origin, app.is_dir)?;
        actions.push(format!("LINKED: {} -> {}", origin.display(), dest.display()));
    }

    Ok(actions)
}

pub fn switch_profile(
    old_profile: &str,
    new_profile: &str,
    config: &SharedAppConfig,
    local: &mut LocalAppConfig,
    roost_dir: &Path,
) -> Result<()> {
    if !config.profiles.contains_key(new_profile) {
        bail!("target profile '{}' does not exist", new_profile);
    }

    let old_dir = roost_dir.join(old_profile);
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

            if let Ok(meta) = fs::symlink_metadata(&origin) {
                if meta.is_symlink() {
                    if let Ok(target) = fs::read_link(&origin) {
                        if target == dest {
                            let _ = fs::remove_file(&origin);
                        }
                    }
                }
            }
        }
    }

    let new_dir = roost_dir.join(new_profile);
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
                create_symlink(&dest, &origin, app.is_dir)?;
            }
        }
    }

    local.active_profile = new_profile.to_string();
    Ok(())
}

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

    create_symlink(&source_dir, &target_dir, true)?;
    Ok(())
}

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

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

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
