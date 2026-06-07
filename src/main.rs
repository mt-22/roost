use clap::{CommandFactory, Parser};
use color_eyre::{Result, eyre::bail};
use dialoguer::{console::style, theme::ColorfulTheme};
use roost::cli::{Cli, Commands, ProfileAction, ProfileCmd};
use roost::{app, git, init, linker, logo, pager, tui};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        None => {
            let (shared, local, roost_dir) = load_configs()?;
            let result = tui::main_view::run(roost_dir, shared, local);
            println!("{}", style(logo::random_exit_banner()).cyan());
            result
        }
        Some(Commands::Init) => cmd_init(),
        Some(Commands::Add { path }) => cmd_add(&path),
        Some(Commands::Remove { app }) => cmd_remove(&app),
        Some(Commands::Sync) => cmd_sync(),
        Some(Commands::Profile(cmd)) => cmd_profile(cmd),
        Some(Commands::Diff) => cmd_diff(),
        Some(Commands::Log) => cmd_log(),
        Some(Commands::Ignore { app, list, pattern }) => cmd_ignore(app, list, pattern),
        Some(Commands::Undo { n }) => cmd_undo(n),
        Some(Commands::Rollback { hash }) => cmd_rollback(&hash),
        Some(Commands::Restore { app }) => cmd_restore(&app),
        Some(Commands::Remote { url }) => cmd_remote(url),
        Some(Commands::Doctor { fix }) => cmd_doctor(fix),
        Some(Commands::Adopt) => cmd_adopt(),
        Some(Commands::Where { app, profile }) => cmd_where(&app, profile),
        Some(Commands::List { profile }) => cmd_list(profile),
        Some(Commands::Save { message }) => cmd_save(message),
        Some(Commands::Status) => cmd_status(),
        Some(Commands::Import { app, from }) => cmd_import(&app, &from),
        Some(Commands::Copy { app, to }) => cmd_copy(&app, &to),
        Some(Commands::Completions { shell }) => cmd_completions(shell),
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

fn cmd_status() -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let profile_name = &local.active_profile;
    let app_count = shared
        .profiles
        .get(profile_name)
        .map(|p| p.apps.len())
        .unwrap_or(0);
    let dirty = git::is_dirty(&roost_dir)?;
    let remote = git::get_remote(&roost_dir)?.unwrap_or_else(|| "none".to_string());
    let last_commit = match git::log(&roost_dir, 1)? {
        commits if !commits.is_empty() => {
            let c = &commits[0];
            let short = &c.hash[..7.min(c.hash.len())];
            format!("{}  {}", short, c.message)
        }
        _ => "none".to_string(),
    };

    println!(
        "{} {}",
        style("Active profile:").bold(),
        style(profile_name).cyan()
    );
    println!(
        "{} {}",
        style("App count:").bold(),
        style(app_count).white().bold()
    );
    println!(
        "{} {}",
        style("Dirty state:").bold(),
        if dirty {
            style("dirty").yellow()
        } else {
            style("clean").green()
        }
    );
    println!("{} {}", style("Remote URL:").bold(), style(&remote).white());
    println!(
        "{} {}",
        style("Last commit:").bold(),
        style(&last_commit).white()
    );
    Ok(())
}

fn cmd_import(app_name: &str, source_profile: &str) -> Result<()> {
    let (mut shared, mut local, roost_dir) = load_configs()?;
    let current_profile = local.active_profile.clone();
    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);

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

    linker::import_from(app_name, source_profile, &current_profile, &roost_dir)?;

    if let Some(profile) = shared.profiles.get_mut(&current_profile) {
        profile.apps.insert(app_name.to_string());
        profile
            .app_sources
            .insert(app_name.to_string(), source_profile.to_string());
    }
    if let Some(app_entry) = shared.apps.get_mut(app_name) {
        app_entry.on_profiles.insert(current_profile.clone());
    }

    app::save_shared(&shared_path, &shared)?;

    let actions = linker::ensure_links(&shared, &mut local, &roost_dir)?;
    app::save_local(&local_path, &local)?;

    git::save(
        &roost_dir,
        &format!("import: {} from {}", app_name, source_profile),
    )?;

    println!(
        "{} '{}' from '{}'",
        style("Imported").green(),
        style(app_name).cyan(),
        style(source_profile).cyan()
    );
    for action in &actions {
        println!("  {}", style(action).dim());
    }

    Ok(())
}

fn cmd_copy(app_name: &str, target_profile: &str) -> Result<()> {
    let (mut shared, local, roost_dir) = load_configs()?;
    let current_profile = local.active_profile;
    let shared_path = app::shared_config_path(&roost_dir);

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

    linker::copy_to(app_name, &current_profile, target_profile, &roost_dir)?;

    if let Some(profile) = shared.profiles.get_mut(target_profile) {
        profile.apps.insert(app_name.to_string());
    }
    if let Some(app_entry) = shared.apps.get_mut(app_name) {
        app_entry.on_profiles.insert(target_profile.to_string());
    }

    app::save_shared(&shared_path, &shared)?;
    git::save(
        &roost_dir,
        &format!("copy: {} to {}", app_name, target_profile),
    )?;

    println!(
        "{} '{}' to '{}'",
        style("Copied").green(),
        style(app_name).cyan(),
        style(target_profile).cyan()
    );

    Ok(())
}

fn cmd_completions(shell: String) -> Result<()> {
    let shell = shell
        .parse::<clap_complete::Shell>()
        .map_err(|_| color_eyre::eyre::eyre!("Unsupported shell: {}", shell))?;
    clap_complete::generate(shell, &mut Cli::command(), "roost", &mut std::io::stdout());
    Ok(())
}

fn format_app(
    shared: &app::SharedAppConfig,
    local: &app::LocalAppConfig,
    app_name: &str,
) -> Option<String> {
    let app = shared.apps.get(app_name)?;
    let kind = if app.is_dir { "dir" } else { "file" };
    let origin = local
        .link_paths
        .get(app_name)
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    Some(format!(
        "{} {} {} {}",
        style(app_name).cyan(),
        style(format!("({})", kind)).dim(),
        style("→").dim(),
        style(origin).dim(),
    ))
}

fn cmd_where(app_name: &str, profile: Option<String>) -> Result<()> {
    let (shared, local, _) = load_configs()?;
    let profile_name = profile.as_deref().unwrap_or(&local.active_profile);
    let profile = shared
        .profiles
        .get(profile_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("Profile '{}' not found.", profile_name))?;
    if !profile.apps.contains(app_name) {
        bail!(
            "App '{}' not found in profile '{}'.",
            app_name,
            profile_name
        );
    }
    let line = format_app(&shared, &local, app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found in config.", app_name))?;
    println!("{}", line);
    Ok(())
}

fn cmd_list(profile: Option<String>) -> Result<()> {
    let (shared, local, _) = load_configs()?;
    let profile_name = profile.as_deref().unwrap_or(&local.active_profile);
    let profile = shared
        .profiles
        .get(profile_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("Profile '{}' not found.", profile_name))?;
    for app_name in &profile.apps {
        if let Some(line) = format_app(&shared, &local, app_name) {
            println!("{}", line);
        }
    }
    Ok(())
}

fn cmd_save(message: Option<String>) -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    if !git::is_dirty(&roost_dir)? {
        println!("{}", style("Nothing to save.").dim());
        return Ok(());
    }
    let msg = if let Some(m) = message {
        m
    } else {
        git::diff_stat(&roost_dir)
            .map(|s| {
                if s.is_empty() {
                    "save: manual save".to_string()
                } else {
                    format!("save: {}", s)
                }
            })
            .unwrap_or_else(|_| "save: manual save".to_string())
    };
    git::save(&roost_dir, &msg)?;
    println!("{}", style("Saved.").green());
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
    let formatted: Vec<String> = commits
        .iter()
        .map(|c| {
            let short = &c.hash[..7.min(c.hash.len())];
            format!(
                "{}  {}  {}",
                style(short).cyan(),
                style(&c.timestamp).dim(),
                style(&c.message).white()
            )
        })
        .collect();
    pager::open(&formatted.join("\n"))
}

fn cmd_ignore(app: Option<String>, list: bool, pattern: Option<String>) -> Result<()> {
    let (mut shared, _, roost_dir) = load_configs()?;
    let shared_path = app::shared_config_path(&roost_dir);

    if list || pattern.is_none() {
        // List current rules
        println!("{}", style("Global ignore patterns:").bold());
        if shared.ignored.is_empty() {
            println!("  (none)");
        } else {
            for p in &shared.ignored {
                println!("  {}", style(p).white());
            }
        }

        let apps_with_ignores: Vec<_> = shared
            .apps
            .iter()
            .filter(|(_, a)| !a.ignore.is_empty())
            .collect();
        if !apps_with_ignores.is_empty() {
            println!();
            println!("{}", style("Per-app ignore patterns:").bold());
            for (name, app) in apps_with_ignores {
                for p in &app.ignore {
                    println!("  {}  {}", style(name).cyan(), style(p).white());
                }
            }
        }
        return Ok(());
    }

    let pattern = pattern.unwrap();

    if let Some(app_name) = app {
        let app_entry = shared
            .apps
            .get_mut(&app_name)
            .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;
        if !app_entry.ignore.contains(&pattern) {
            app_entry.ignore.push(pattern.clone());
            println!(
                "{} {} {}",
                style("Added per-app ignore").green(),
                style(&app_name).cyan(),
                style(&pattern).white()
            );
        } else {
            println!(
                "{} {} {}",
                style("Pattern already exists for").yellow(),
                style(&app_name).cyan(),
                style(&pattern).white()
            );
        }
    } else {
        if shared.ignored.insert(pattern.clone()) {
            println!(
                "{} {}",
                style("Added global ignore").green(),
                style(&pattern).white()
            );
        } else {
            println!(
                "{} {}",
                style("Pattern already exists:").yellow(),
                style(&pattern).white()
            );
        }
    }

    app::save_shared(&shared_path, &shared)?;
    roost::gitignore::regenerate(&roost_dir, &shared.ignored, &shared.apps)?;
    git::save(&roost_dir, &format!("ignore: added {}", pattern))?;
    println!("{}", style("Updated .gitignore").green());
    Ok(())
}

fn cmd_remote(url: Option<String>) -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    match url {
        Some(u) => git::set_remote(&roost_dir, &u),
        None => {
            match git::get_remote(&roost_dir)? {
                Some(remote) => println!("{}", style(remote).white()),
                None => println!("{}", style("No remote configured.").yellow()),
            }
            Ok(())
        }
    }
}

fn cmd_undo(n: Option<usize>) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let count = n.unwrap_or(1);
    let profile_name = local.active_profile.clone();
    let hash = format!("HEAD~{}", count);
    git::safe_rollback(&roost_dir, &hash, &shared, &local, &profile_name)?;
    println!(
        "{} {} commit(s) with app preservation.",
        style("Undid").green(),
        style(count).white().bold()
    );
    Ok(())
}

fn cmd_rollback(hash: &str) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let profile_name = local.active_profile.clone();
    git::safe_rollback(&roost_dir, hash, &shared, &local, &profile_name)?;
    println!(
        "{} {}.",
        style("Rolled back to").green(),
        style(hash).white().bold()
    );
    Ok(())
}

fn cmd_add(path: &std::path::Path) -> Result<()> {
    let (mut shared, mut local, roost_dir) = load_configs()?;
    let profile_name = local.active_profile.clone();
    let pdir = app::profile_dir(&roost_dir, &profile_name);
    if !path.exists() {
        bail!("Path '{}' does not exist.", path.display());
    }
    let is_dir = path.is_dir();
    let file_name = path
        .file_name()
        .ok_or_else(|| color_eyre::eyre::eyre!("Cannot determine file name from path."))?
        .to_string_lossy()
        .to_string();
    let app_name_raw = if file_name.starts_with('.') && file_name.len() > 1 {
        &file_name[1..]
    } else if file_name.is_empty() {
        &file_name
    } else {
        file_name.as_str()
    };
    let app_name = app::sanitize_app_name(app_name_raw);
    if app_name.is_empty() {
        bail!("Cannot determine app name from path.");
    }
    if shared.apps.contains_key(&app_name) {
        bail!("App '{}' already managed.", app_name);
    }
    linker::ingest(path, &pdir, &app_name, is_dir)?;
    shared.apps.insert(
        app_name.clone(),
        app::Application {
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
        .insert(app_name.to_string(), path.to_path_buf());
    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);
    app::save_shared(&shared_path, &shared)?;
    app::save_local(&local_path, &local)?;
    let actions = linker::ensure_links(&shared, &mut local, &roost_dir)?;
    for action in actions {
        println!("{}", style(action).dim());
    }
    git::save(&roost_dir, &format!("add: {}", app_name))?;
    println!("{} {}", style("Added").green(), style(app_name).cyan());
    Ok(())
}

fn cmd_remove(app_name: &str) -> Result<()> {
    let (mut shared, mut local, roost_dir) = load_configs()?;
    let profile_name = local.active_profile.clone();
    let app_entry = shared
        .apps
        .get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;
    let origin = local
        .link_paths
        .get(app_name)
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
    println!("{} {}", style("Removed").green(), style(app_name).cyan());
    Ok(())
}

fn cmd_sync() -> Result<()> {
    let (_, _, roost_dir) = load_configs()?;
    let result = git::sync(&roost_dir, git::ConflictPreference::Local)?;

    // Reload configs since sync may have changed roost.toml via structural merge
    let (shared, mut local, _) = load_configs()?;
    let actions = linker::ensure_links(&shared, &mut local, &roost_dir)?;
    // Save local.toml in case ensure_links auto-discovered link_paths
    let local_path = app::local_config_path(&roost_dir);
    let _ = app::save_local(&local_path, &local);

    match result {
        git::SyncResult::Clean => println!(
            "{}",
            style("Sync complete. Changes pushed to origin.").green()
        ),
        git::SyncResult::ConfigConflict { resolved } => {
            println!(
                "{}",
                style("Sync complete. Config conflicts resolved and pushed to origin.").yellow()
            );
            for name in &resolved {
                println!("  - {}", style(name).yellow().dim());
            }
        }
        git::SyncResult::FileConflict { file_conflicts, backups, .. } => {
            println!(
                "{}",
                style("Sync encountered file conflicts with remote. Rebase was aborted; your local changes are intact.").yellow()
            );
            println!(
                "{}",
                style("To resolve: cd ~/.roost && git status, then fix conflicts and re-run sync.").yellow()
            );
            if !file_conflicts.is_empty() {
                println!("{}", style("Conflicting files:").yellow());
                for f in &file_conflicts {
                    println!("  - {}", style(f).yellow().dim());
                }
            }
            if !backups.is_empty() {
                println!("{}", style("Local backups saved to ~/.roost/.backups/sync/").yellow());
            }
        }
    }

    if !actions.is_empty() {
        println!("{}", style("Link actions:").cyan());
        for action in &actions {
            println!("  {}", style(action).dim());
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
                let active = *name == local.active_profile;
                let marker = if active {
                    style("*").cyan().bold().to_string()
                } else {
                    style(" ").to_string()
                };
                println!(
                    "{} {}",
                    marker,
                    if active {
                        style(name).cyan().bold()
                    } else {
                        style(name)
                    },
                );
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
            println!(
                "{} {}",
                style("Switched to profile:").green(),
                style(&name).cyan()
            );
            Ok(())
        }
        ProfileAction::Add { name, from } => {
            if shared.profiles.contains_key(&name) {
                bail!("Profile '{}' already exists.", name);
            }
            let new_profile = if let Some(source_name) = from {
                let source = shared.profiles.get(&source_name).ok_or_else(|| {
                    color_eyre::eyre::eyre!("Source profile '{}' not found.", source_name)
                })?;
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
            println!(
                "{} {}",
                style("Created profile:").green(),
                style(&name).cyan()
            );
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
                let fallback = shared
                    .profiles
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());
                local.active_profile = fallback.clone();
                app::save_local(&local_path, &local)?;
                println!(
                    "{}",
                    style(format!(
                        "Active profile was deleted. Falling back to '{}'.",
                        fallback
                    ))
                    .yellow()
                );
            }
            app::save_shared(&shared_path, &shared)?;
            git::save(&roost_dir, &format!("profile: delete {}", name))?;
            println!(
                "{} {}",
                style("Deleted profile:").green(),
                style(&name).cyan()
            );
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
                let updates: Vec<(String, String)> = profile
                    .app_sources
                    .iter()
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
            println!(
                "{} {} {}",
                style("Renamed profile:").green(),
                style(&old).cyan(),
                style(format!("→ {}", new)).dim()
            );
            Ok(())
        }
    }
}

fn cmd_restore(app_name: &str) -> Result<()> {
    let (shared, local, roost_dir) = load_configs()?;
    let profile_name = &local.active_profile;
    let app_entry = shared
        .apps
        .get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("App '{}' not found.", app_name))?;
    let origin = local
        .link_paths
        .get(app_name)
        .ok_or_else(|| color_eyre::eyre::eyre!("No link path for '{}'.", app_name))?;
    let pdir = app::profile_dir(&roost_dir, profile_name);
    linker::restore(origin, &pdir, app_name, app_entry.is_dir)?;
    println!("{} {}", style("Restored").green(), style(app_name).cyan());
    Ok(())
}

fn cmd_doctor(fix: bool) -> Result<()> {
    let (shared, mut local, roost_dir) = load_configs()?;
    let mut had_error = false;
    let mut had_warn = false;
    let profile_name = &local.active_profile;

    println!("{}", style("== Config Consistency ==").bold());
    let active_profile = match shared.profiles.get(profile_name) {
        Some(p) => p,
        None => {
            println!(
                "{} Active profile '{}' not found in shared config",
                style("[ERROR]").red().bold(),
                style(profile_name).cyan()
            );
            bail!("Active profile not found.");
        }
    };

    for app_name in &active_profile.apps {
        if !shared.apps.contains_key(app_name) {
            println!(
                "{} App '{}' in profile '{}' but missing from apps",
                style("[ERROR]").red().bold(),
                style(app_name).cyan(),
                style(profile_name).dim()
            );
            had_error = true;
        }
    }

    for (app_name, app) in &shared.apps {
        if app.on_profiles.is_empty() {
            println!(
                "{} App '{}' not in any profile",
                style("[WARN]").yellow().bold(),
                style(app_name).cyan()
            );
            had_warn = true;
        }
    }

    for (prof_name, profile) in &shared.profiles {
        for (app_name, source) in &profile.app_sources {
            if !shared.profiles.contains_key(source) {
                println!(
                    "{} Profile '{}' app '{}' references non-existent source '{}'",
                    style("[ERROR]").red().bold(),
                    style(prof_name).cyan(),
                    style(app_name).cyan(),
                    style(source).dim()
                );
                had_error = true;
            }
            if !shared.apps.contains_key(app_name) {
                println!(
                    "{} Profile '{}' references non-existent app '{}'",
                    style("[ERROR]").red().bold(),
                    style(prof_name).cyan(),
                    style(app_name).cyan()
                );
                had_error = true;
            }
        }
    }

    println!("\n{}", style("== Symlink Health ==").bold());
    let link_statuses = linker::check_links(&shared, &local, &roost_dir)?;
    for status in &link_statuses {
        match status {
            linker::LinkStatus::Ok {
                app,
                origin,
                target,
            } => {
                println!(
                    "{} {}: {} {} {}",
                    style("[OK]").green().bold(),
                    style(app).cyan(),
                    style(origin.display()).dim(),
                    style("→").dim(),
                    style(target.display()).dim()
                );
            }
            linker::LinkStatus::Missing {
                app,
                origin,
                target,
            } => {
                println!(
                    "{} {}: {} {} {}",
                    style("[MISSING]").yellow().bold(),
                    style(app).cyan(),
                    style(origin.display()).dim(),
                    style("→").dim(),
                    style(target.display()).dim()
                );
                had_warn = true;
            }
            linker::LinkStatus::Broken {
                app,
                origin,
                actual,
                expected,
            } => {
                println!(
                    "{} {}: {} {} {} (expected {})",
                    style("[ERROR]").red().bold(),
                    style(app).cyan(),
                    style(origin.display()).dim(),
                    style("→").dim(),
                    style(actual.display()).dim(),
                    style(expected.display()).dim()
                );
                had_error = true;
            }
            linker::LinkStatus::Conflict { app, origin } => {
                println!(
                    "{} {}: {} exists as real file/dir",
                    style("[CONFLICT]").red().bold(),
                    style(app).cyan(),
                    style(origin.display()).dim()
                );
                had_error = true;
            }
            linker::LinkStatus::NoLinkPath { app } => {
                println!(
                    "{} {}: no link path configured",
                    style("[WARN]").yellow().bold(),
                    style(app).cyan()
                );
                had_warn = true;
            }
        }
    }

    println!("\n{}", style("== Orphan Detection ==").bold());
    let orphans = linker::find_orphans(&shared, &roost_dir)?;
    for orphan in &orphans {
        let kind = if orphan.is_dir { "dir" } else { "file" };
        println!(
            "{} Profile '{}' has orphaned {}: '{}'",
            style("[WARN]").yellow().bold(),
            style(&orphan.profile).cyan(),
            kind,
            style(&orphan.name).cyan()
        );
        had_warn = true;
    }
    if orphans.is_empty() {
        println!("{}", style("No orphans found.").green());
    }

    if fix {
        let actions = linker::ensure_links(&shared, &mut local, &roost_dir)?;
        if !actions.is_empty() {
            println!("\n{}", style("== Auto-fix ==").bold());
            for action in &actions {
                println!(
                    "{} {}",
                    style("[FIXED]").green().bold(),
                    style(action).green()
                );
            }
        }
    }

    println!();
    if had_error {
        bail!("Doctor found errors.");
    } else if had_warn {
        println!(
            "{}",
            style("Doctor found warnings. Run `roost adopt` to register orphaned apps.").yellow()
        );
    } else {
        println!("{}", style("All checks passed.").green());
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
        println!("{}", style("No orphaned apps found.").dim());
        return Ok(());
    }

    let items: Vec<String> = orphans
        .iter()
        .map(|(name, is_dir)| format!("{} ({})", name, if *is_dir { "dir" } else { "file" }))
        .collect();

    let selected = dialoguer::MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select orphaned apps to adopt")
        .items(&items)
        .interact()?;

    if selected.is_empty() {
        println!("No apps selected.");
        return Ok(());
    }

    for idx in &selected {
        let (name, is_dir) = &orphans[*idx];
        shared.apps.insert(
            name.clone(),
            app::Application {
                primary_config: None,
                on_profiles: {
                    let mut s = std::collections::BTreeSet::new();
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
        println!("{} {}", style("Adopted").green(), style(name).cyan());
    }

    app::save_shared(&shared_path, &shared)?;
    app::save_local(&local_path, &local)?;
    git::save(&roost_dir, "adopt: registered orphaned apps")?;

    println!(
        "{}",
        style("Done. Run `roost doctor` to verify symlinks.").green()
    );
    Ok(())
}
