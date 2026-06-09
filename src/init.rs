use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use color_eyre::Result;
use dialoguer::{Confirm, Input, MultiSelect, Select, console::style, theme::ColorfulTheme};

use crate::{app, app_selector, data, git, linker, logo, os_detect, scanner};

fn roost_theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

fn get_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

pub fn run_wizard() -> Result<()> {
    let roost_dir = app::roost_dir();
    println!(
        "  {} {}",
        style("Roost directory:").cyan().bold(),
        style(roost_dir.display()).dim()
    );

    let shared_path = app::shared_config_path(&roost_dir);
    let local_path = app::local_config_path(&roost_dir);

    let existing_shared = shared_path.exists();
    let existing_local = local_path.exists();

    if existing_shared && existing_local {
        let proceed = Confirm::with_theme(&roost_theme())
            .with_prompt(format!(
                "Roost is already initialized at {}. Add a new profile?",
                roost_dir.display()
            ))
            .default(true)
            .interact()?;
        if !proceed {
            println!("{}", style("Aborting.").yellow());
            return Ok(());
        }
    }

    // Reconstruct local state when roost.toml exists but local.toml is missing
    if existing_shared && !existing_local {
        println!(
            "{}",
            style("Existing shared config found. Reconstructing local state...")
                .cyan()
                .bold()
        );
        return reconstruct_local(&roost_dir, shared_path.clone(), local_path.clone());
    }

    let theme = roost_theme();
    let remote_url: String = Input::with_theme(&theme)
        .with_prompt("Git remote URL (leave empty to skip)")
        .allow_empty(true)
        .interact()?;
    let remote_url = if remote_url.trim().is_empty() {
        None
    } else {
        Some(remote_url.trim().to_string())
    };

    if !existing_shared
        && !existing_local
        && let Some(ref url) = remote_url
    {
        match git::hydrate_existing_remote(&roost_dir, url)? {
            git::RemoteHydration::Hydrated => {
                println!(
                    "{}",
                    style("Existing remote found. Hydrating Roost state...")
                        .cyan()
                        .bold()
                );
                return reconstruct_local(&roost_dir, shared_path.clone(), local_path.clone());
            }
            git::RemoteHydration::EmptyRemote => {}
        }
    }

    let hostname = get_hostname();
    let profile_name: String = Input::with_theme(&theme)
        .with_prompt("Profile name:")
        .default(hostname)
        .interact()?;

    let mut config = if existing_shared {
        let mut cfg = app::load_shared(&shared_path)?;
        cfg.profiles
            .entry(profile_name.clone())
            .or_insert(app::Profile {
                apps: BTreeSet::new(),
                app_sources: BTreeMap::new(),
            });
        if remote_url.is_some() {
            cfg.remote = remote_url.clone().or(cfg.remote.clone());
        }
        cfg
    } else {
        let mut cfg = app::SharedAppConfig {
            remote: remote_url.clone(),
            profiles: BTreeMap::new(),
            apps: BTreeMap::new(),
            ignored: BTreeSet::new(),
        };
        cfg.profiles.insert(
            profile_name.clone(),
            app::Profile {
                apps: BTreeSet::new(),
                app_sources: BTreeMap::new(),
            },
        );
        cfg
    };

    let local = app::LocalAppConfig {
        active_profile: profile_name.clone(),
        os_info: os_detect::detect(),
        link_paths: BTreeMap::new(),
    };

    let ignore_items: Vec<String> = data::DEFAULT_IGNORE_PATTERNS
        .iter()
        .map(|s| s.to_string())
        .collect();
    let ignore_defaults: Vec<bool> = ignore_items.iter().map(|_| true).collect();
    let selected_ignore = MultiSelect::with_theme(&theme)
        .with_prompt("Select ignore patterns")
        .items(&ignore_items)
        .defaults(&ignore_defaults)
        .interact()?;
    let ignored: BTreeSet<String> = selected_ignore
        .into_iter()
        .map(|i| ignore_items[i].clone())
        .collect();
    config.ignored = ignored;

    std::fs::create_dir_all(&roost_dir)?;
    app::save_local(&local_path, &local)?;

    let ignored_set: HashSet<String> = config.ignored.iter().cloned().collect();

    let home = dirs::home_dir().expect("no home directory");
    let sources = scanner::default_scan_sources(&home);
    let all_items = scanner::scan_sources(&sources, &ignored_set);

    crate::tui::init();

    let selected =
        match app_selector::run_selection_tui(all_items, &home, &crate::tui::SHOULD_EXIT, true)? {
            app_selector::TuiResult::Selected(items) => items,
            app_selector::TuiResult::Aborted => {
                println!("{}", style("Aborted. No changes made.").yellow());
                return Ok(());
            }
        };

    let mut local = local;

    if selected.is_empty() {
        println!(
            "{}",
            style("No apps selected. Continuing with empty config.").yellow()
        );
    } else {
        let pdir = app::profile_dir(&roost_dir, &profile_name);
        let mut failures = Vec::new();

        let home = dirs::home_dir().expect("no home directory");
        for item in &selected {
            let app_name_raw = if item.name.starts_with('.') && item.name.len() > 1 {
                &item.name[1..]
            } else {
                &item.name
            };
            let app_name_raw = if app_name_raw.is_empty() {
                &item.name
            } else {
                app_name_raw
            };
            let app_name = app::sanitize_app_name(app_name_raw);

            // Skip paths outside home or with parent-dir references
            if let Err(e) = linker::validate_path_in_home(&item.path, &home) {
                println!(
                    "  {} {}",
                    style("✗").red().bold(),
                    style(format!("skipped {}: {}", app_name, e)).red()
                );
                failures.push((app_name.to_string(), e));
                continue;
            }

            match linker::ingest(
                &item.path,
                &pdir,
                &app_name,
                item.item_type == scanner::ItemType::Dir,
            ) {
                Ok(()) => {
                    config.apps.insert(
                        app_name.to_string(),
                        app::Application {
                            primary_config: None,
                            on_profiles: {
                                let mut s = BTreeSet::new();
                                s.insert(profile_name.clone());
                                s
                            },
                            is_dir: item.item_type == scanner::ItemType::Dir,
                            ignore: Vec::new(),
                        },
                    );
                    if let Some(profile) = config.profiles.get_mut(&profile_name) {
                        profile.apps.insert(app_name.to_string());
                    }
                    local
                        .link_paths
                        .insert(app_name.to_string(), item.path.clone());
                    println!(
                        "  {} {}",
                        style("✓").green().bold(),
                        style(format!("ingested {}", app_name)).green()
                    );
                }
                Err(e) => {
                    println!(
                        "  {} {}",
                        style("✗").red().bold(),
                        style(format!("failed to ingest {}: {}", app_name, e)).red()
                    );
                    failures.push((app_name.to_string(), e));
                }
            }
        }

        if !failures.is_empty() {
            println!();
            println!("{}", style("Failures:").red().bold());
            for (name, err) in &failures {
                println!("  {}: {}", style(name).red(), style(err).red().dim());
            }
        }

        app::save_local(&local_path, &local)?;
    }

    app::guess_primary_configs(&roost_dir, &profile_name, &mut config, &local)?;
    app::validate_shared(&config)?;
    app::save_shared(&shared_path, &config)?;
    println!("{}", style("Config written.").green());

    crate::gitignore::regenerate(&roost_dir, &config.ignored, &config.apps)?;

    let actions = linker::ensure_links(&config, &mut local, &roost_dir)?;
    for action in &actions {
        println!("{}", style(action).dim());
    }

    git::init(&roost_dir)?;
    if let Some(ref url) = remote_url {
        git::set_remote(&roost_dir, url)?;
    }
    git::save(&roost_dir, "init: roost initialized")?;
    println!("{}", style("Git repository initialized.").green());

    println!("{}", style(logo::ROOST_LOGO).cyan());
    let app_count = config.apps.len();
    println!(
        "{} Profile: {}, {} apps managed.",
        style("Roost initialized!").cyan().bold(),
        style(&profile_name).white().bold(),
        style(app_count).green().bold()
    );

    Ok(())
}

/// Reconstruct local.toml from an existing roost.toml (e.g. after a fresh clone).
fn reconstruct_local(
    roost_dir: &Path,
    shared_path: std::path::PathBuf,
    local_path: std::path::PathBuf,
) -> Result<()> {
    let theme = roost_theme();
    let mut config = app::load_shared(&shared_path)?;

    let remote_url: String = Input::with_theme(&theme)
        .with_prompt("Git remote URL (leave empty to keep existing)")
        .allow_empty(true)
        .interact()?;
    let remote_url = if remote_url.trim().is_empty() {
        config.remote.clone()
    } else {
        Some(remote_url.trim().to_string())
    };
    if remote_url.is_some() {
        config.remote = remote_url.clone();
    }

    // Let user pick the active profile from existing profiles
    let profile_name = if config.profiles.is_empty() {
        let hostname = get_hostname();
        println!(
            "{}",
            style("No profiles found in shared config. Creating new profile.").yellow()
        );
        config.profiles.insert(
            hostname.clone(),
            app::Profile {
                apps: BTreeSet::new(),
                app_sources: BTreeMap::new(),
            },
        );
        hostname
    } else if config.profiles.len() == 1 {
        config.profiles.keys().next().unwrap().clone()
    } else {
        let profile_names: Vec<String> = config.profiles.keys().cloned().collect();
        let idx = Select::with_theme(&theme)
            .with_prompt("Choose active profile")
            .items(&profile_names)
            .default(0)
            .interact()?;
        profile_names[idx].clone()
    };

    let home = dirs::home_dir().expect("no home directory");
    let mut local = app::LocalAppConfig {
        active_profile: profile_name.clone(),
        os_info: os_detect::detect(),
        link_paths: BTreeMap::new(),
    };

    // Discover original paths for each app in the active profile
    let pdir = app::profile_dir(roost_dir, &profile_name);
    if let Some(profile) = config.profiles.get(&profile_name) {
        if !profile.apps.is_empty() {
            println!(
                "{}",
                style("Discovering original paths for managed apps...").cyan()
            );
        }
        for app_name in &profile.apps {
            let discovered = discover_app_origin(app_name, &pdir, roost_dir, &config, &home);
            if let Some(path) = discovered {
                println!(
                    "  {} {} -> {}",
                    style("✓").green().bold(),
                    style(app_name).cyan(),
                    style(path.display()).dim()
                );
                local.link_paths.insert(app_name.clone(), path);
            } else {
                println!(
                    "  {} {} — {}",
                    style("?").yellow().bold(),
                    style(app_name).cyan(),
                    style("no path discovered, skipped").dim()
                );
            }
        }
    }

    app::save_local(&local_path, &local)?;
    println!("{}", style("Local config written.").green());

    let actions = linker::ensure_links(&config, &mut local, roost_dir)?;
    if !actions.is_empty() {
        println!("{}", style("Link actions:").cyan());
        for action in &actions {
            println!("  {}", style(action).dim());
        }
    }

    if !roost_dir.join(".git").exists() {
        git::init(roost_dir)?;
        if let Some(ref url) = remote_url {
            git::set_remote(roost_dir, url)?;
        }
        git::save(roost_dir, "init: roost initialized")?;
        println!("{}", style("Git repository initialized.").green());
    } else if let Some(ref url) = remote_url {
        // Ensure remote is set even if git already exists
        if git::get_remote(roost_dir)?.is_none() {
            git::set_remote(roost_dir, url)?;
        }
    }

    println!("{}", style(logo::ROOST_LOGO).cyan());
    let app_count = config.apps.len();
    println!(
        "{} Profile: {}, {} apps managed.",
        style("Roost initialized!").cyan().bold(),
        style(&profile_name).white().bold(),
        style(app_count).green().bold()
    );

    Ok(())
}

/// Attempt to discover the original path for an already-managed app.
/// Checks for existing symlinks pointing into the roost profile dir first,
/// then falls back to common default locations.
fn discover_app_origin(
    app_name: &str,
    pdir: &Path,
    _roost_dir: &Path,
    config: &app::SharedAppConfig,
    home: &Path,
) -> Option<std::path::PathBuf> {
    // 1. Check if a symlink already exists at a likely path pointing into roost
    let candidates = vec![
        home.join(".config").join(app_name),
        home.join(format!(".{}", app_name)),
        home.join(app_name),
        home.join("Library")
            .join("Application Support")
            .join(app_name),
    ];

    for candidate in &candidates {
        if candidate.is_symlink() {
            if let Ok(target) = std::fs::read_link(candidate) {
                let expected = if let Some(app) = config.apps.get(app_name) {
                    linker::app_dest(pdir, app_name, app.is_dir)
                } else {
                    pdir.join(app_name)
                };
                if target == expected {
                    return Some(candidate.clone());
                }
            }
        }
    }

    // 2. Check if a real file/dir exists at a likely path (user hasn't set up symlinks yet)
    // Only auto-discover if it matches the app's expected type
    for candidate in &candidates {
        if candidate.exists() && !candidate.is_symlink() {
            if let Some(app) = config.apps.get(app_name) {
                let is_dir = candidate.is_dir();
                if is_dir == app.is_dir {
                    return Some(candidate.clone());
                }
            }
        }
    }

    None
}
