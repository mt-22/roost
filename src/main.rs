mod app;
mod git;
mod init;
mod linker;
mod os_detect;
mod pager;
mod scanner;
mod tui;
use color_eyre;
use color_eyre::eyre::eyre;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("init") => return init::init_system(),
        Some("sync") => return run_sync(),
        Some("profile") => return run_profile(&args),
        Some("diff") => return run_diff(),
        Some("log") => return run_log(),
        Some("undo") => return run_undo(&args),
        Some("rollback") => return run_rollback(&args),
        Some("remove") => return run_remove(&args),
        Some("where") => return run_where(&args),
        Some("restore") => return run_restore(),
        Some("remote") => return run_remote(&args),
        Some("add") => return run_add(&args),
        Some("doctor") => return run_doctor(),
        Some("status") => return run_status(),
        Some("help") | Some("--help") | Some("-h") => {
            print_help();
            return Ok(());
        }
        Some(other) if other.starts_with('-') => {
            eprintln!("Unknown option: {}", other);
            print_help();
            std::process::exit(1);
        }
        _ => {}
    }

    run_main_view()
}

fn print_help() {
    println!("roost — dotfile manager");
    println!();
    println!("Usage: roost <command> [args]");
    println!();
    println!("Commands:");
    println!("  init                Initialize roost (interactive)");
    println!("  add <path>          Ingest a path into roost");
    println!("  sync                Commit and sync with remote");
    println!("  status              Show managed apps and link status");
    println!("  profile <subcmd>    Manage profiles (add|list|switch|delete|rename)");
    println!("  where <app>         Print where an app's files live");
    println!("  restore             Repair all symlinks");
    println!("  remote [set <url>]  Show or set the git remote URL");
    println!("  doctor              Run diagnostics on config and symlinks");
    println!("  diff                Show uncommitted changes");
    println!("  log                 Show recent commits");
    println!("  undo [n]            Undo last n commit(s) (destructive)");
    println!("  rollback <hash>     Rollback to a specific commit (destructive)");
    println!("  remove <app>        Stop managing an app, restore files");
    println!("  help                Show this help message");
    println!();
    println!("Run without a command to launch the TUI.");
}

fn roost_paths() -> color_eyre::Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)>
{
    let roost_dir = std::env::var("ROOST_DIR")
        .map(|p| std::path::PathBuf::from(p))
        .unwrap_or_else(|_| {
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
            home.join(".roost")
        });
    let roost_config = roost_dir.join("roost.toml");
    let local_config = roost_dir.join("local.toml");

    if !roost_config.exists() || !local_config.exists() {
        return Err(eyre!("Roost is not initialized. Run `roost init` first."));
    }

    Ok((roost_dir, roost_config, local_config))
}

fn run_sync() -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    git::sync(&roost_dir)?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    let mut local = app::LocalAppConfig::load(&local_config_path)?;
    app::migrate_link_paths_if_needed(&roost_config, &mut local, &local_config_path)?;
    linker::ensure_links(&config, &local, &roost_dir);
    Ok(())
}

fn run_profile(args: &[String]) -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    let mut config = app::SharedAppConfig::load(&roost_config)?;
    let mut local = app::LocalAppConfig::load(&local_config_path)?;

    match args.get(2).map(|s| s.as_str()) {
        Some("add") => {
            let name = args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: roost profile add <name> [--empty]");
                std::process::exit(1);
            });
            let empty = args.get(4).map(|s| s.as_str()) == Some("--empty");
            let template_name: Option<String> = if empty {
                None
            } else {
                Some(local.active_profile.clone())
            };
            let count = app::add_profile(
                name,
                &roost_dir,
                &mut config,
                &roost_config,
                &mut local,
                &local_config_path,
                template_name.as_deref(),
            )?;
            if let Some(ref t) = template_name {
                println!(
                    "Created profile '{}' with {} app(s) from '{}'.",
                    name, count, t
                );
            } else {
                println!("Created empty profile '{}'.", name);
            }
            Ok(())
        }
        Some("list") | None => {
            println!("Profiles:");
            for name in config.profiles.keys() {
                let marker = if *name == local.active_profile {
                    " * "
                } else {
                    "   "
                };
                println!("{}{}", marker, name);
            }
            Ok(())
        }
        Some("switch") => {
            let name = args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: roost profile switch <name>");
                std::process::exit(1);
            });
            if !config.profiles.contains_key(name) {
                eprintln!("Profile '{}' does not exist.", name);
                std::process::exit(1);
            }
            let old_profile = local.active_profile.clone();
            linker::switch_links(&old_profile, name, &config, &local, &roost_dir);
            local.active_profile = name.to_string();
            local.save(&local_config_path)?;
            linker::ensure_links(&config, &local, &roost_dir);
            println!("Switched to profile '{}'.", name);
            Ok(())
        }
        Some("delete") => {
            let name = args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: roost profile delete <name>");
                std::process::exit(1);
            });
            if !config.profiles.contains_key(name) {
                eprintln!("Profile '{}' does not exist.", name);
                std::process::exit(1);
            }
            if name == local.active_profile {
                eprintln!("Cannot delete the active profile. Switch first.");
                std::process::exit(1);
            }
            let app_count = config.profiles.get(name).map(|p| p.apps.len()).unwrap_or(0);
            eprint!(
                "Delete profile '{}'? {} app(s) will be restored. [y/N] ",
                name, app_count
            );
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer).ok();
            if answer.trim().to_lowercase() != "y" {
                eprintln!("Aborted.");
                return Ok(());
            }
            app::delete_profile(
                name,
                &roost_dir,
                &mut config,
                &roost_config,
                &mut local,
                &local_config_path,
            )?;
            println!("Deleted profile '{}'.", name);
            Ok(())
        }
        Some("rename") => {
            let old_name = args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: roost profile rename <old> <new>");
                std::process::exit(1);
            });
            let new_name = args.get(4).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: roost profile rename <old> <new>");
                std::process::exit(1);
            });

            if !config.profiles.contains_key(old_name) {
                eprintln!("Profile '{}' does not exist.", old_name);
                std::process::exit(1);
            }
            if config.profiles.contains_key(new_name) {
                eprintln!("Profile '{}' already exists.", new_name);
                std::process::exit(1);
            }
            if old_name == new_name {
                eprintln!("Old and new names are the same.");
                std::process::exit(1);
            }

            let old_dir = roost_dir.join(old_name);
            let new_dir = roost_dir.join(new_name);
            if old_dir.exists() {
                std::fs::rename(&old_dir, &new_dir)?;
            }

            let profile = config.profiles.remove(old_name).unwrap();
            config.profiles.insert(new_name.to_string(), profile);

            for (_app_name, app) in config.apps.iter_mut() {
                if let Some(pos) = app.on_profiles.iter().position(|p| p == old_name) {
                    app.on_profiles[pos] = new_name.to_string();
                }
            }

            for (_prof_name, profile) in config.profiles.iter_mut() {
                let keys_to_update: Vec<String> = profile
                    .app_sources
                    .iter()
                    .filter(|(_, v)| *v == old_name)
                    .map(|(k, _)| k.clone())
                    .collect();
                for key in keys_to_update {
                    profile.app_sources.insert(key, new_name.to_string());
                }
            }

            if local.active_profile == old_name {
                local.active_profile = new_name.to_string();
                local.save(&local_config_path)?;
            }

            config.save(&roost_config)?;

            if git::is_git_repo(&roost_dir) {
                let _ = git::auto_commit(
                    &roost_dir,
                    &format!("renamed profile {} to {}", old_name, new_name),
                );
            }

            println!("Renamed profile '{}' to '{}'.", old_name, new_name);
            Ok(())
        }
        Some(other) => {
            eprintln!("Unknown profile command: {}", other);
            eprintln!("Usage: roost profile [add|list|switch|delete|rename] [name]");
            std::process::exit(1);
        }
    }
}

fn run_main_view() -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config) = roost_paths()?;
    let mut local = app::LocalAppConfig::load(&local_config)?;
    // Migrate link_paths from old roost.toml format → local.toml (idempotent).
    app::migrate_link_paths_if_needed(&roost_config, &mut local, &local_config)?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    // Resave so any remaining ~/... conversions are written out.
    config.save(&roost_config)?;

    tui::main_view::run_main_view(config, roost_dir, roost_config, local_config, local)
}

fn run_diff() -> color_eyre::Result<()> {
    let (roost_dir, _, _) = roost_paths()?;
    if !git::is_dirty(&roost_dir)? {
        println!("No uncommitted changes.");
        return Ok(());
    }
    let diff = git::diff_text(&roost_dir)?;
    pager::show_in_pager(&diff)
}

fn run_log() -> color_eyre::Result<()> {
    let (roost_dir, _, _) = roost_paths()?;
    let entries = git::log(&roost_dir, 20)?;
    if entries.is_empty() {
        println!("No commits yet.");
        return Ok(());
    }
    for entry in &entries {
        println!(
            "{}  {:>12}  {}",
            entry.short_hash, entry.date, entry.message
        );
    }
    Ok(())
}

fn run_undo(args: &[String]) -> color_eyre::Result<()> {
    let (roost_dir, _, _) = roost_paths()?;
    let n = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);
    let entries = git::log(&roost_dir, n)?;
    if entries.len() < n {
        eprintln!("Not enough commits to undo.");
        std::process::exit(1);
    }
    eprint!(
        "Undo last {} commit(s)? This will PERMANENTLY DISCARD those changes. [y/N] ",
        n
    );
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    if answer.trim().to_lowercase() != "y" {
        eprintln!("Aborted.");
        return Ok(());
    }
    git::undo(&roost_dir, n)?;
    println!("Undone {} commit(s).", n);
    Ok(())
}

fn run_rollback(args: &[String]) -> color_eyre::Result<()> {
    let (roost_dir, _, _) = roost_paths()?;
    let hash = args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
        eprintln!("Usage: roost rollback <hash>");
        std::process::exit(1);
    });
    eprint!(
        "Rollback to {}? This is destructive — working tree will be reset. [y/N] ",
        hash
    );
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    if answer.trim().to_lowercase() != "y" {
        eprintln!("Aborted.");
        return Ok(());
    }
    git::rollback(&roost_dir, hash)?;
    println!("Rolled back to {}.", hash);
    Ok(())
}

fn run_remove(args: &[String]) -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    let mut config = app::SharedAppConfig::load(&roost_config)?;
    let local = app::LocalAppConfig::load(&local_config_path)?;

    let app_name = args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
        eprintln!("Usage: roost remove <app>");
        eprintln!("Run 'roost status' to see managed apps.");
        std::process::exit(1);
    });

    let Some(app) = config.apps.get(app_name).cloned() else {
        eprintln!("App '{}' is not managed by roost.", app_name);
        std::process::exit(1);
    };

    eprint!(
        "Remove '{}'? Files will be restored to their original location. [y/N] ",
        app_name
    );
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    if answer.trim().to_lowercase() != "y" {
        eprintln!("Aborted.");
        return Ok(());
    }

    if let Some(link_path) = local.link_paths.get(app_name) {
        for profile_name in &app.on_profiles {
            let profile_dir = roost_dir.join(profile_name);
            if let Err(e) = linker::unlink(link_path, &profile_dir, &roost_dir) {
                eprintln!("  warn: could not unlink: {}", e);
            }
            if let Some(profile) = config.profiles.get_mut(profile_name) {
                profile.apps.remove(app_name);
            }
        }
    }

    config.apps.remove(app_name);
    config.save(&roost_config)?;

    if git::is_git_repo(&roost_dir) {
        let _ = git::auto_commit(&roost_dir, &format!("removed app {}", app_name));
    }

    println!("Removed '{}'.", app_name);
    Ok(())
}

fn run_status() -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    let local = app::LocalAppConfig::load(&local_config_path)?;

    println!("Profile: {}", local.active_profile);
    println!("Apps managed: {}", config.apps.len());

    let mut broken = Vec::new();
    for (name, app) in &config.apps {
        let status = if let Some(link_path) = local.link_paths.get(name) {
            if !link_path.exists() && !link_path.is_symlink() {
                "missing".to_string()
            } else if link_path.is_symlink() {
                let target = std::fs::read_link(link_path).unwrap_or_default();
                if target.starts_with(&roost_dir) {
                    if link_path.exists() {
                        "linked".to_string()
                    } else {
                        broken.push(name.clone());
                        "broken symlink".to_string()
                    }
                } else {
                    "external symlink".to_string()
                }
            } else {
                "not linked".to_string()
            }
        } else {
            "not on this device".to_string()
        };
        let primary = app
            .primary_config
            .as_ref()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let primary_info = if primary.is_empty() {
            String::new()
        } else {
            format!(" (primary: {})", primary)
        };
        println!("  {} [{}]{}", name, status, primary_info);
    }

    if !broken.is_empty() {
        println!();
        eprintln!(
            "Warning: broken symlinks detected for: {}",
            broken.join(", ")
        );
    }

    Ok(())
}

fn run_add(args: &[String]) -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    let mut config = app::SharedAppConfig::load(&roost_config)?;
    let mut local = app::LocalAppConfig::load(&local_config_path)?;

    let raw_path = args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
        eprintln!("Usage: roost add <path>");
        std::process::exit(1);
    });

    let path = std::path::PathBuf::from(raw_path);
    if !path.exists() {
        eprintln!("Path '{}' does not exist.", raw_path);
        std::process::exit(1);
    }

    if linker::is_roost_symlink(&path, &roost_dir) {
        let app_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                eprintln!("Cannot determine app name from path.");
                std::process::exit(1);
            });
        let active_profile = local.active_profile.clone();
        if let Some(existing) = config.apps.get_mut(&app_name) {
            if !existing.on_profiles.contains(&active_profile) {
                existing.on_profiles.push(active_profile.clone());
            }
        }
        if let Some(profile) = config.profiles.get_mut(&active_profile) {
            profile.apps.insert(app_name.clone());
        }
        config.save(&roost_config)?;
        local.save(&local_config_path)?;
        println!("Added '{}'.", app_name);
        return Ok(());
    }

    let canonical = path.canonicalize()?;
    let app_name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            eprintln!("Cannot determine app name from path.");
            std::process::exit(1);
        });

    let active_profile = local.active_profile.clone();
    let profile_dir = roost_dir.join(&active_profile);

    if let Err(e) = linker::ingest(&canonical, &profile_dir, &roost_dir) {
        eprintln!("Could not ingest '{}': {}", raw_path, e);
        std::process::exit(1);
    }

    let entry = scanner::SourceEntry {
        path: canonical.clone(),
        name: app_name.clone(),
    };
    let application = scanner::entry_to_application(&entry, &config.ignored, &active_profile)?;

    if let Some(profile) = config.profiles.get_mut(&active_profile) {
        profile.apps.insert(app_name.clone());
    }
    local.link_paths.insert(app_name.clone(), canonical.clone());

    if let Some(existing) = config.apps.get_mut(&app_name) {
        if !existing.on_profiles.contains(&active_profile) {
            existing.on_profiles.push(active_profile.clone());
        }
    } else {
        config.apps.insert(app_name.clone(), application);
    }

    config.save(&roost_config)?;
    local.save(&local_config_path)?;

    if git::is_git_repo(&roost_dir) {
        let _ = git::auto_commit(&roost_dir, &format!("added app {}", app_name));
    }

    println!("Added '{}'.", app_name);
    Ok(())
}

fn run_where(args: &[String]) -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    let local = app::LocalAppConfig::load(&local_config_path)?;

    let app_name = args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
        eprintln!("Usage: roost where <app>");
        std::process::exit(1);
    });

    let Some(_app) = config.apps.get(app_name) else {
        eprintln!("App '{}' is not managed by roost.", app_name);
        std::process::exit(1);
    };

    if let Some(link_path) = local.link_paths.get(app_name) {
        println!("  link path: {}", link_path.display());
    } else {
        println!("  link path: (not on this device)");
    }

    for profile_name in &config.apps.get(app_name).unwrap().on_profiles {
        let prof_dir = roost_dir.join(profile_name);
        if let Ok(dest) = linker::roost_dest(
            &prof_dir,
            local
                .link_paths
                .get(app_name)
                .unwrap_or(&std::path::PathBuf::new()),
        ) {
            println!("  profile '{}': {}", profile_name, dest.display());
        }
    }

    Ok(())
}

fn run_restore() -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    let local = app::LocalAppConfig::load(&local_config_path)?;
    linker::ensure_links(&config, &local, &roost_dir);
    println!("Links restored.");
    Ok(())
}

fn run_remote(args: &[String]) -> color_eyre::Result<()> {
    let (roost_dir, roost_config, _) = roost_paths()?;

    match args.get(2).map(|s| s.as_str()) {
        Some("set") => {
            let url = args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                eprintln!("Usage: roost remote set <url>");
                std::process::exit(1);
            });

            if git::is_git_repo(&roost_dir) {
                let remotes = git::git_output(&roost_dir, &["remote"])?;
                if remotes.trim().contains("origin") {
                    git::git(&roost_dir, &["remote", "set-url", "origin", url])?;
                } else {
                    git::git(&roost_dir, &["remote", "add", "origin", url])?;
                }
            }

            let mut config = app::SharedAppConfig::load(&roost_config)?;
            config.remote = Some(url.to_string());
            config.save(&roost_config)?;

            let _ = git::auto_commit(&roost_dir, &format!("set remote to {}", url));
            println!("Remote set to {}", url);
        }
        Some(other) => {
            eprintln!("Unknown remote command: {}", other);
            eprintln!("Usage: roost remote set <url>");
            std::process::exit(1);
        }
        None => {
            let config = app::SharedAppConfig::load(&roost_config)?;
            match &config.remote {
                Some(url) => println!("{}", url),
                None => println!("No remote configured."),
            }
        }
    }

    Ok(())
}

fn run_doctor() -> color_eyre::Result<()> {
    let (roost_dir, roost_config, local_config_path) = roost_paths()?;
    let config = app::SharedAppConfig::load(&roost_config)?;
    let local = app::LocalAppConfig::load(&local_config_path)?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut info = Vec::new();

    for (name, link_path) in &local.link_paths {
        if link_path.is_symlink() {
            let target = std::fs::read_link(link_path).unwrap_or_default();
            if target.starts_with(&roost_dir) && !link_path.exists() {
                errors.push(format!(
                    "broken symlink: {} → {}",
                    link_path.display(),
                    target.display()
                ));
            }
        } else if !link_path.exists() {
            errors.push(format!(
                "missing link path for '{}': {}",
                name,
                link_path.display()
            ));
        } else if !link_path.is_symlink() && config.apps.contains_key(name) {
            warnings.push(format!(
                "'{}' exists but is not a symlink (not managed by roost): {}",
                name,
                link_path.display()
            ));
        }
    }

    for (prof_name, profile) in &config.profiles {
        let prof_dir = roost_dir.join(prof_name);
        for app_name in &profile.apps {
            if let Some(link_path) = local.link_paths.get(app_name)
                && let Ok(dest) = linker::roost_dest(&prof_dir, link_path)
                && !dest.exists() && !dest.is_symlink()
            {
                warnings.push(format!(
                    "app '{}' has no files in profile '{}' (expected at {})",
                    app_name,
                    prof_name,
                    dest.display()
                ));
            }
        }
    }

    for (prof_name, profile) in &config.profiles {
        let prof_dir = roost_dir.join(prof_name);
        if prof_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&prof_dir)
        {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "misc" {
                    if let Ok(misc_entries) = std::fs::read_dir(prof_dir.join("misc")) {
                        for me in misc_entries.flatten() {
                            let mname = me.file_name().to_string_lossy().to_string();
                            if !profile.apps.contains(&mname)
                                && !profile.app_sources.contains_key(&mname)
                            {
                                info.push(format!(
                                    "orphaned in profile '{}/misc': {}",
                                    prof_name, mname
                                ));
                            }
                        }
                    }
                } else if !profile.apps.contains(&name)
                    && !profile.app_sources.contains_key(&name)
                {
                    info.push(format!("orphaned in profile '{}': {}", prof_name, name));
                }
            }
        }
    }

    for (prof_name, profile) in &config.profiles {
        for (app_name, source_profile) in &profile.app_sources {
            if !config.profiles.contains_key(source_profile) {
                errors.push(format!(
                    "app '{}' in profile '{}' sources from '{}', which doesn't exist",
                    app_name, prof_name, source_profile
                ));
                continue;
            }
            let source_prof = config.profiles.get(source_profile).unwrap();
            if !source_prof.apps.contains(app_name) {
                errors.push(format!(
                    "app '{}' in profile '{}' sources from '{}', but '{}' doesn't contain that app",
                    app_name, prof_name, source_profile, source_profile
                ));
            }
        }
    }

    for (prof_name, profile) in &config.profiles {
        for app_name in &profile.apps {
            if !config.apps.contains_key(app_name) {
                errors.push(format!(
                    "profile '{}' references app '{}' which doesn't exist in config.apps",
                    prof_name, app_name
                ));
            }
        }
    }
    for (app_name, app) in &config.apps {
        for prof in &app.on_profiles {
            if !config.profiles.contains_key(prof) {
                warnings.push(format!(
                    "app '{}' references profile '{}' which doesn't exist",
                    app_name, prof
                ));
            } else if !config.profiles.get(prof).unwrap().apps.contains(app_name) {
                warnings.push(format!(
                    "app '{}' lists profile '{}' in on_profiles, but that profile doesn't list the app",
                    app_name, prof
                ));
            }
        }
    }

    if !errors.is_empty() {
        println!("Errors:");
        for e in &errors {
            eprintln!("  ✗ {}", e);
        }
    }
    if !warnings.is_empty() {
        println!("Warnings:");
        for w in &warnings {
            eprintln!("  ⚠ {}", w);
        }
    }
    if !info.is_empty() {
        println!("Info:");
        for i in &info {
            println!("  ℹ {}", i);
        }
    }

    if errors.is_empty() && warnings.is_empty() && info.is_empty() {
        println!("All checks passed. No issues found.");
    }

    if !errors.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
