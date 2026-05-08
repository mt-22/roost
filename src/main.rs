use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};
use color_eyre::{Result, eyre::bail};
use roost::{app, git, init, linker, pager};

#[derive(Parser)]
#[command(name = "roost", version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Add { path: PathBuf },
    Remove { app: String },
    Sync,
    Profile(ProfileCmd),
    Diff,
    Log,
    Undo { n: Option<usize> },
    Rollback { hash: String },
    Restore { app: String },
    Remote { url: Option<String> },
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    Adopt,
    Where { app: String },
}

#[derive(Args)]
struct ProfileCmd {
    #[command(subcommand)]
    action: ProfileAction,
}

#[derive(Subcommand)]
enum ProfileAction {
    List,
    Switch { name: String },
    Add { name: String, #[arg(long)] from: Option<String> },
    Delete { name: String },
    Rename { old: String, new: String },
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        None => {
            println!("Main TUI not yet implemented — use `roost <command>`");
            Ok(())
        }
        Some(Commands::Init) => cmd_init(),
        Some(Commands::Add { path }) => cmd_add(&path),
        Some(Commands::Remove { app }) => cmd_remove(&app),
        Some(Commands::Sync) => cmd_sync(),
        Some(Commands::Profile(cmd)) => cmd_profile(cmd),
        Some(Commands::Diff) => cmd_diff(),
        Some(Commands::Log) => cmd_log(),
        Some(Commands::Undo { n }) => cmd_undo(n),
        Some(Commands::Rollback { hash }) => cmd_rollback(&hash),
        Some(Commands::Restore { app }) => cmd_restore(&app),
        Some(Commands::Remote { url }) => cmd_remote(url),
        Some(Commands::Doctor { fix }) => cmd_doctor(fix),
        Some(Commands::Adopt) => cmd_adopt(),
        Some(Commands::Where { app }) => cmd_where(&app),
    }
}

fn load_configs() -> Result<(app::SharedAppConfig, app::LocalAppConfig, PathBuf)> {
    let roost_dir = app::roost_dir();
    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);
    if !shared_path.exists() || !local_path.exists() {
        bail!("Roost not initialized. Run `roost init` first.");
    }
    let shared = app::load_shared(&shared_path)?;
    let local = app::load_local(&local_path)?;
    Ok((shared, local, roost_dir))
}

fn cmd_init() -> Result<()> {
    init::run_wizard()
}

fn cmd_where(app_name: &str) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let app_entry = shared.apps.get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found in config.", app_name))?;
    let pdir = app::profile_dir(&roost_dir, &local.active_profile);
    let dest = linker::app_dest(&pdir, app_name, app_entry.is_dir);
    println!("{}", dest.display());
    Ok(())
}

fn cmd_diff() -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    let output = git::diff(&roost_dir)?;
    pager::open(&output)
}

fn cmd_log() -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    let commits = git::log(&roost_dir, 20)?;
    let formatted: Vec<String> = commits.iter()
        .map(|c| {
            let short = &c.hash[..7.min(c.hash.len())];
            format!("{}  {}  {}", short, c.timestamp, c.message)
        })
        .collect();
    pager::open(&formatted.join("\n"))
}

fn cmd_remote(url: Option<String>) -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    match url {
        Some(u) => git::set_remote(&roost_dir, &u),
        None => {
            match git::get_remote(&roost_dir)? {
                Some(remote) => println!("{}", remote),
                None => println!("No remote configured."),
            }
            Ok(())
        }
    }
}

fn cmd_undo(n: Option<usize>) -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    let count = n.unwrap_or(1);
    git::undo(&roost_dir, count)?;
    println!("Undid {} commit(s).", count);
    Ok(())
}

fn cmd_rollback(hash: &str) -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    git::rollback(&roost_dir, hash)?;
    println!("Rolled back to {}.", hash);
    Ok(())
}

fn cmd_add(path: &PathBuf) -> Result<()> {
    let (mut shared, mut local, roost_dir) = load_configs()?;
    let profile_name = local.active_profile.clone();
    let pdir = app::profile_dir(&roost_dir, &profile_name);
    if !path.exists() {
        bail!("Path '{}' does not exist.", path.display());
    }
    let is_dir = path.is_dir();
    let file_name = path.file_name()
        .ok_or_else(|| color_eyre::eyre::eyre!("Cannot determine file name from path."))?
        .to_string_lossy()
        .to_string();
    let app_name = if file_name.starts_with('.') && file_name.len() > 1 {
        &file_name[1..]
    } else if file_name.is_empty() {
        &file_name
    } else {
        file_name.as_str()
    };
    if app_name.is_empty() {
        bail!("Cannot determine app name from path.");
    }
    if shared.apps.contains_key(app_name) {
        bail!("App '{}' already managed.", app_name);
    }
    linker::ingest(path, &pdir, app_name, is_dir)?;
    shared.apps.insert(app_name.to_string(), app::Application {
        primary_config: None,
        on_profiles: {
            let mut s = BTreeSet::new();
            s.insert(profile_name.clone());
            s
        },
        is_dir,
    });
    if let Some(profile) = shared.profiles.get_mut(&profile_name) {
        profile.apps.insert(app_name.to_string());
    }
    local.link_paths.insert(app_name.to_string(), path.clone());
    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);
    app::save_shared(&shared_path, &shared)?;
    app::save_local(&local_path, &local)?;
    let actions = linker::ensure_links(&shared, &local, &roost_dir)?;
    for action in actions {
        println!("{}", action);
    }
    git::save(&roost_dir, &format!("add: {}", app_name))?;
    println!("Added {}", app_name);
    Ok(())
}

fn cmd_remove(app_name: &str) -> Result<()> {
    let (mut shared, mut local, roost_dir) = load_configs()?;
    let profile_name = local.active_profile.clone();
    let app_entry = shared.apps.get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;
    let origin = local.link_paths.get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("No link path for '{}'.", app_name))?;
    let pdir = app::profile_dir(&roost_dir, &profile_name);
    linker::unlink(origin, &pdir, app_name, app_entry.is_dir)?;
    shared.apps.remove(app_name);
    if let Some(profile) = shared.profiles.get_mut(&profile_name) {
        profile.apps.remove(app_name);
    }
    local.link_paths.remove(app_name);
    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);
    app::save_shared(&shared_path, &shared)?;
    app::save_local(&local_path, &local)?;
    git::save(&roost_dir, &format!("remove: {}", app_name))?;
    println!("Removed {}", app_name);
    Ok(())
}

fn cmd_sync() -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    let result = git::sync(&roost_dir, git::ConflictPreference::Local)?;
    match result {
        git::SyncResult::Clean => println!("Sync complete."),
        git::SyncResult::ConfigConflict { resolved } => {
            println!("Config conflicts resolved:");
            for name in &resolved {
                println!("  - {}", name);
            }
        }
        git::SyncResult::FileConflict { .. } => {
            println!("File conflicts detected. Manual resolution required.");
        }
    }
    Ok(())
}

fn cmd_profile(cmd: ProfileCmd) -> Result<()> {
    let (mut shared, mut local, roost_dir) = load_configs()?;
    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);

    match cmd.action {
        ProfileAction::List => {
            for name in shared.profiles.keys() {
                let marker = if *name == local.active_profile { "*" } else { " " };
                println!("{} {}", marker, name);
            }
            Ok(())
        }
        ProfileAction::Switch { name } => {
            if !shared.profiles.contains_key(&name) {
                bail!("Profile '{}' does not exist.", name);
            }
            let old = local.active_profile.clone();
            linker::switch_profile(&old, &name, &shared, &mut local, &roost_dir)?;
            app::save_local(&local_path, &local)?;
            println!("Switched to profile: {}", name);
            Ok(())
        }
        ProfileAction::Add { name, from } => {
            if shared.profiles.contains_key(&name) {
                bail!("Profile '{}' already exists.", name);
            }
            let new_profile = if let Some(source_name) = from {
                let source = shared.profiles.get(&source_name)
                    .ok_or_else(|| color_eyre::eyre::eyre!("Source profile '{}' not found.", source_name))?;
                app::Profile {
                    apps: source.apps.clone(),
                    app_sources: source.app_sources.clone(),
                }
            } else {
                app::Profile {
                    apps: BTreeSet::new(),
                    app_sources: BTreeMap::new(),
                }
            };
            shared.profiles.insert(name.clone(), new_profile);
            app::save_shared(&shared_path, &shared)?;
            git::save(&roost_dir, &format!("profile: add {}", name))?;
            println!("Created profile: {}", name);
            Ok(())
        }
        ProfileAction::Delete { name } => {
            if !shared.profiles.contains_key(&name) {
                bail!("Profile '{}' does not exist.", name);
            }
            for app_name in shared.profiles[&name].apps.iter() {
                if let Some(app) = shared.apps.get_mut(app_name) {
                    app.on_profiles.remove(&name);
                    if app.on_profiles.is_empty() {
                        shared.apps.remove(app_name);
                    }
                }
            }
            shared.profiles.remove(&name);
            if local.active_profile == name {
                let fallback = shared.profiles.keys().next()
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());
                local.active_profile = fallback.clone();
                app::save_local(&local_path, &local)?;
                println!("Active profile was deleted. Falling back to '{}'.", fallback);
            }
            app::save_shared(&shared_path, &shared)?;
            git::save(&roost_dir, &format!("profile: delete {}", name))?;
            println!("Deleted profile: {}", name);
            Ok(())
        }
        ProfileAction::Rename { old, new } => {
            if !shared.profiles.contains_key(&old) {
                bail!("Profile '{}' does not exist.", old);
            }
            if shared.profiles.contains_key(&new) {
                bail!("Profile '{}' already exists.", new);
            }
            let profile = shared.profiles.remove(&old).unwrap();
            shared.profiles.insert(new.clone(), profile);
            for app in shared.apps.values_mut() {
                if app.on_profiles.remove(&old) {
                    app.on_profiles.insert(new.clone());
                }
            }
            for profile in shared.profiles.values_mut() {
                let updates: Vec<(String, String)> = profile.app_sources.iter()
                    .filter(|(_, src)| *src == &old)
                    .map(|(app_name, _)| (app_name.clone(), new.clone()))
                    .collect();
                for (app_name, new_src) in updates {
                    profile.app_sources.insert(app_name, new_src);
                }
            }
            if local.active_profile == old {
                local.active_profile = new.clone();
                app::save_local(&local_path, &local)?;
            }
            app::save_shared(&shared_path, &shared)?;
            git::save(&roost_dir, &format!("profile: rename {} -> {}", old, new))?;
            println!("Renamed profile: {} -> {}", old, new);
            Ok(())
        }
    }
}

fn cmd_restore(app_name: &str) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let profile_name = &local.active_profile;
    let app_entry = shared.apps.get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;
    let origin = local.link_paths.get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("No link path for '{}'.", app_name))?;
    let pdir = app::profile_dir(&roost_dir, profile_name);
    linker::restore(origin, &pdir, app_name, app_entry.is_dir)?;
    println!("Restored {}", app_name);
    Ok(())
}

fn cmd_doctor(fix: bool) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let mut had_error = false;
    let mut had_warn = false;
    let profile_name = &local.active_profile;

    println!("== Config Consistency ==");
    let active_profile = match shared.profiles.get(profile_name) {
        Some(p) => p,
        None => {
            println!("[ERROR] Active profile '{}' not found in shared config", profile_name);
            bail!("Active profile not found.");
        }
    };

    for app_name in &active_profile.apps {
        if !shared.apps.contains_key(app_name) {
            println!("[ERROR] App '{}' in profile '{}' but missing from apps", app_name, profile_name);
            had_error = true;
        }
    }

    for (app_name, app) in &shared.apps {
        if app.on_profiles.is_empty() {
            println!("[WARN] App '{}' not in any profile", app_name);
            had_warn = true;
        }
    }

    for (prof_name, profile) in &shared.profiles {
        for (app_name, source) in &profile.app_sources {
            if !shared.profiles.contains_key(source) {
                println!("[ERROR] Profile '{}' app '{}' references non-existent source '{}'", prof_name, app_name, source);
                had_error = true;
            }
            if !shared.apps.contains_key(app_name) {
                println!("[ERROR] Profile '{}' references non-existent app '{}'", prof_name, app_name);
                had_error = true;
            }
        }
    }

    println!("\n== Symlink Health ==");
    let link_statuses = linker::check_links(&shared, &local, &roost_dir)?;
    for status in &link_statuses {
        match status {
            linker::LinkStatus::Ok { app, origin, target } => {
                println!("[OK] {}: {} -> {}", app, origin.display(), target.display());
            }
            linker::LinkStatus::Missing { app, origin, target } => {
                println!("[MISSING] {}: {} -> {}", app, origin.display(), target.display());
                had_warn = true;
            }
            linker::LinkStatus::Broken { app, origin, actual, expected } => {
                println!("[ERROR] {}: {} -> {} (expected {})", app, origin.display(), actual.display(), expected.display());
                had_error = true;
            }
            linker::LinkStatus::Conflict { app, origin } => {
                println!("[CONFLICT] {}: {} exists as real file/dir", app, origin.display());
                had_error = true;
            }
            linker::LinkStatus::NoLinkPath { app } => {
                println!("[WARN] {}: no link path configured", app);
                had_warn = true;
            }
        }
    }

    println!("\n== Orphan Detection ==");
    let orphans = linker::find_orphans(&shared, &roost_dir)?;
    for orphan in &orphans {
        let kind = if orphan.is_dir { "dir" } else { "file" };
        println!("[WARN] Profile '{}' has orphaned {}: '{}'", orphan.profile, kind, orphan.name);
        had_warn = true;
    }
    if orphans.is_empty() {
        println!("No orphans found.");
    }

    if fix {
        let actions = linker::ensure_links(&shared, &local, &roost_dir)?;
        if !actions.is_empty() {
            println!("\n== Auto-fix ==");
            for action in &actions {
                println!("[FIXED] {}", action);
            }
        }
    }

    println!();
    if had_error {
        bail!("Doctor found errors.");
    } else if had_warn {
        println!("Doctor found warnings. Run `roost adopt` to register orphaned apps.");
    } else {
        println!("All checks passed.");
    }
    Ok(())
}

fn cmd_adopt() -> Result<()> {
    let (mut shared, local, roost_dir) = load_configs()?;
    let profile_name = local.active_profile.clone();
    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);
    let pdir = app::profile_dir(&roost_dir, &profile_name);

    let mut orphans: Vec<(String, bool)> = Vec::new();

    if pdir.exists() {
        for entry in std::fs::read_dir(&pdir)?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == linker::MISC_DIR_NAME || name.starts_with('.') {
                continue;
            }
            if !shared.apps.contains_key(&name) {
                let is_dir = entry.file_type()?.is_dir();
                orphans.push((name, is_dir));
            }
        }
        let misc_dir = pdir.join(linker::MISC_DIR_NAME);
        if misc_dir.exists() {
            for entry in std::fs::read_dir(&misc_dir)?.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !shared.apps.contains_key(&name) {
                    let is_dir = entry.file_type()?.is_dir();
                    orphans.push((name, is_dir));
                }
            }
        }
    }

    if orphans.is_empty() {
        println!("No orphaned apps found.");
        return Ok(());
    }

    let items: Vec<String> = orphans.iter()
        .map(|(name, is_dir)| format!("{} ({})", name, if *is_dir { "dir" } else { "file" }))
        .collect();

    let selected = dialoguer::MultiSelect::new()
        .with_prompt("Select orphaned apps to adopt")
        .items(&items)
        .interact()?;

    if selected.is_empty() {
        println!("No apps selected.");
        return Ok(());
    }

    for idx in &selected {
        let (name, is_dir) = &orphans[*idx];
        shared.apps.insert(name.clone(), app::Application {
            primary_config: None,
            on_profiles: {
                let mut s = std::collections::BTreeSet::new();
                s.insert(profile_name.clone());
                s
            },
            is_dir: *is_dir,
        });
        if let Some(profile) = shared.profiles.get_mut(&profile_name) {
            profile.apps.insert(name.clone());
        }
        println!("Adopted {}", name);
    }

    app::save_shared(&shared_path, &shared)?;
    app::save_local(&local_path, &local)?;
    git::save(&roost_dir, "adopt: registered orphaned apps")?;

    println!("Done. Run `roost doctor` to verify symlinks.");
    Ok(())
}
