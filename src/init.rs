use color_eyre::eyre::{Ok, eyre};
use dialoguer::{Confirm, Input, MultiSelect, console, console::Style, theme::ColorfulTheme};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::PathBuf,
    process::Command,
};

use crate::app::{LocalAppConfig, Profile, SharedAppConfig};
use crate::git;
use crate::linker;
use crate::logo;
use crate::scanner;
use crate::tui;
use crate::tui::state::OnboardingContext;

const SEPARATOR_LENGTH: usize = 60;

fn roost_theme() -> ColorfulTheme {
    ColorfulTheme {
        prompt_style: Style::new().white().bold(),
        prompt_prefix: console::style("?".to_string()).cyan().bold(),
        success_prefix: console::style("✓".to_string()).green().bold(),
        error_prefix: console::style("✗".to_string()).red().bold(),
        active_item_style: Style::new().cyan().bold(),
        inactive_item_style: Style::new().white(),
        active_item_prefix: console::style("›".to_string()).cyan().bold(),
        picked_item_prefix: console::style("✓".to_string()).green().bold(),
        unpicked_item_prefix: console::style(" ".to_string()),
        checked_item_prefix: console::style("✓".to_string()).green().bold(),
        unchecked_item_prefix: console::style("○".to_string()).white(),
        values_style: Style::new().green(),
        hint_style: Style::new().white().dim(),
        ..ColorfulTheme::default()
    }
}

fn separator() -> String {
    "─".repeat(SEPARATOR_LENGTH)
}

const SUGGESTED_IGNORES: &[&str] = &[
    "node_modules",
    ".git",
    ".DS_Store",
    "target",
    "__pycache__",
    ".cache",
    "*.log",
    "*.swp",
    "*.swo",
    "*.sqlite",
    "*.sqlite-wal",
    "*.sqlite-shm",
    "*.socket",
    "*.socket.lock",
    "*.lock",
    "bun.lock",
    "package-lock.json",
    ".Trash",
    ".undodir",
    "Thumbs.db",
];

/// Prompt user to select from a list and optionally add custom entries.
fn select_ignores(theme: &ColorfulTheme) -> color_eyre::Result<HashSet<String>> {
    let items: Vec<String> = SUGGESTED_IGNORES.iter().map(|s| s.to_string()).collect();

    let indices = MultiSelect::with_theme(theme)
        .with_prompt("Select ignore patterns (space to toggle, enter to confirm)")
        .items(&items)
        .defaults(&[true].repeat(items.len()))
        .interact()?;

    let mut values: HashSet<String> = indices.into_iter().map(|i| items[i].clone()).collect();

    println!("{}", separator());
    loop {
        let input: String = Input::with_theme(theme)
            .with_prompt("Add custom ignore pattern (leave blank to finish)")
            .allow_empty(true)
            .interact_text()?;

        if input.is_empty() {
            break;
        }
        if !values.insert(input.clone()) {
            println!("  Already added: {}", input);
        }
    }

    println!("{}", separator());
    println!("Configured {} ignore patterns.", values.len());
    println!("{}", separator());

    Ok(values)
}

pub fn init_system() -> color_eyre::Result<()> {
    let theme = roost_theme();
    let roost_dir = if let std::result::Result::Ok(env_dir) = env::var("ROOST_DIR") {
        PathBuf::from(env_dir)
    } else {
        let home = dirs::home_dir().expect("Failed to find home directory");
        home.join(".roost")
    };
    let roost_config = roost_dir.join("roost.toml");
    let local_config = roost_dir.join("local.toml");

    // =========================================
    // CHECK IF ALREADY SETUP
    // =========================================

    if !roost_dir.exists() {
        fs::create_dir_all(&roost_dir)?;
    } else if roost_config.exists() && local_config.exists() {
        println!(
            "Roost is already fully initialized in {}!",
            roost_dir.display()
        );
        println!("Run `roost` to open the TUI, or `roost help` for available commands.");
        return Ok(());
    } else if local_config.exists() && !roost_config.exists() {
        println!(
            "Partial initialization detected in {} — local.toml exists but roost.toml is missing.",
            roost_dir.display()
        );
        println!(
            "This usually means init was interrupted after setting up the profile but before selecting apps."
        );
        if Confirm::with_theme(&theme)
            .with_prompt("Resume setup from app selection?")
            .default(true)
            .interact()?
        {
            println!("Resuming setup...");
            println!("{}", separator());
        } else if Confirm::with_theme(&theme)
            .with_prompt("Start fresh? (DANGEROUS — all existing files will be lost)")
            .interact()?
        {
            fs::remove_dir_all(&roost_dir)?;
            fs::create_dir_all(&roost_dir)?;
        } else {
            return Err(eyre!(
                "Aborted — resolve manually in {}.",
                roost_dir.display()
            ));
        }
    } else if fs::read_dir(&roost_dir)?.next().is_some() {
        println!(
            "{} is not empty! roost may be partially initialized.",
            roost_dir.display()
        );
        if Confirm::with_theme(&theme)
            .with_prompt("Would you like to overwrite it? (DANGEROUS)")
            .interact()?
        {
            fs::remove_dir_all(&roost_dir)?;
            fs::create_dir_all(&roost_dir)?;
        } else {
            return Err(eyre!("Aborted — roost directory not clean."));
        }
    }

    println!("{}", separator());

    // =========================================
    // GIT REMOTE SETUP (before TUI)
    // =========================================

    let mut remote: Option<String> = None;
    if Confirm::with_theme(&theme)
        .with_prompt("Set up a remote repository for multi-device syncing? (can do later)")
        .default(false)
        .interact()?
    {
        println!("{}", separator());
        println!("Provide your repository URL (e.g. from GitHub's 'Code' button).");
        println!("{}", separator());

        let remote_url: String = Input::with_theme(&theme)
            .with_prompt("Remote URL")
            .interact_text()?;

        if !git::is_git_repo(&roost_dir) {
            println!("Initializing repo...");
            git::git(&roost_dir, &["init"])?;
        }
        println!("Adding remote origin...");
        git::git(&roost_dir, &["remote", "add", "origin", &remote_url])?;
        let branch = crate::git::git_output(&roost_dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "main".to_string());
        let branch = if branch.is_empty() {
            "main".to_string()
        } else {
            branch
        };
        println!("Pulling from remote...");
        if let Err(e) = git::git(&roost_dir, &["pull", "origin", &branch]) {
            println!("Could not pull from remote: {}", e);
            println!("Continuing with fresh setup...");
        } else {
            println!("Done!");
        }
        remote = Some(remote_url);
    } else if !git::is_git_repo(&roost_dir) {
        println!("Initializing git repo (no remote)...");
        git::git(&roost_dir, &["init"])?;
    }

    println!("{}", separator());

    // =========================================
    // PROFILE NAME
    // =========================================

    let hostname_output = Command::new("hostname")
        .output()
        .expect("failed to execute `hostname`");
    let hostname = String::from_utf8_lossy(hostname_output.stdout.trim_ascii()).to_string();

    let profile_name: String = Input::with_theme(&theme)
        .with_prompt("Profile name for this device")
        .default(hostname)
        .interact_text()?;

    println!("{}", separator());

    // =========================================
    // LOAD EXISTING roost.toml OR PROMPT IGNORES
    // =========================================

    let existing_config = if roost_config.exists() {
        match SharedAppConfig::load(&roost_config) {
            std::result::Result::Ok(cfg) => {
                println!("Found existing roost.toml — using its configuration.");
                Some(cfg)
            }
            Err(e) => {
                println!("Warning: could not parse existing roost.toml: {}", e);
                println!("Starting fresh.");
                None
            }
        }
    } else {
        None
    };

    let (ignored, existing_app_paths) = if let Some(ref cfg) = existing_config {
        // Use ignores from existing config, skip the prompt.
        // Try to load existing local config so we can surface known link paths
        // for apps already managed on this device (e.g. after migration).
        let known_paths: std::collections::HashMap<String, PathBuf> =
            LocalAppConfig::load(&local_config)
                .map(|l| l.link_paths)
                .unwrap_or_default();
        let app_paths: Vec<PathBuf> = cfg
            .apps
            .keys()
            .filter_map(|name| known_paths.get(name).cloned())
            .collect();
        println!(
            "Loaded {} ignore patterns from roost.toml.",
            cfg.ignored.len()
        );
        println!("Loaded {} existing managed apps.", app_paths.len());
        (cfg.ignored.clone(), app_paths)
    } else {
        let ignored = select_ignores(&theme)?;
        (ignored, Vec::<PathBuf>::new())
    };

    // =========================================
    // SOURCES (determined internally)
    // =========================================

    let sources = scanner::get_likely_sources();
    println!("Detected sources:");
    for s in &sources {
        println!("  {}", scanner::source_label(s));
    }

    // =========================================
    // WRITE local.toml
    // =========================================

    // link_paths populated after app selection below — written on second save
    let mut local_app_config = LocalAppConfig {
        active_profile: profile_name.clone(),
        os_info: crate::os_detect::detect(),
        link_paths: std::collections::HashMap::new(),
    };
    let local_toml_string = toml::to_string(&local_app_config)?;
    fs::write(&local_config, &local_toml_string)?;
    println!("Wrote {}", local_config.display());

    println!("Writing .gitignore");
    let _ = fs::write(&roost_dir.join(".gitignore"), "local.toml\n");

    println!("{}", separator());

    // =========================================
    // PHASE 2: TUI APP SELECTION
    // =========================================

    let ctx = OnboardingContext {
        profile_name: profile_name.clone(),
        sources,
        ignored: ignored.clone(),
        existing_app_paths,
    };

    let selections = tui::run_onboarding(ctx)?;

    if selections.is_empty() {
        println!("No applications selected. You can run `roost init` again or manage apps later.");
        return Ok(());
    }

    // =========================================
    // SYMLINK SELECTIONS INTO ROOST
    // =========================================

    let profile_dir = roost_dir.join(&profile_name);
    println!("Linking selections into {}...", profile_dir.display());
    let mut succeeded: Vec<&scanner::SourceEntry> = Vec::new();
    for entry in &selections {
        if let Err(e) = linker::ingest(&entry.path, &profile_dir, &roost_dir) {
            eprintln!("  warn: could not ingest {}: {}", entry.name, e);
        } else {
            succeeded.push(entry);
        }
    }

    // =========================================
    // BUILD & WRITE roost.toml
    // =========================================

    let mut apps: HashMap<String, crate::app::Application> = HashMap::new();
    for entry in &succeeded {
        let app = scanner::entry_to_application(entry, &ignored, &profile_name)?;
        // Record this device's link path in local config
        local_app_config
            .link_paths
            .insert(app.name.clone(), entry.path.clone());
        apps.insert(app.name.clone(), app);
    }
    // Save local.toml again now that link_paths are populated
    local_app_config.save(&local_config)?;

    println!("{}", separator());
    println!("Managing {} applications:", apps.len());
    for name in apps.keys() {
        println!("  ● {}", name);
    }

    // Merge profiles: keep existing profiles, add/update this device's profile
    let mut profiles = existing_config.map(|cfg| cfg.profiles).unwrap_or_default();

    let profile = Profile {
        apps: apps.keys().cloned().collect(),
        app_sources: std::collections::HashMap::new(),
    };
    profiles.insert(profile_name.clone(), profile);

    let has_remote = remote.is_some();

    let shared_config = SharedAppConfig {
        remote,
        profiles,
        apps,
        ignored,
    };

    shared_config.save(&roost_config)?;
    println!("Wrote {}", roost_config.display());
    println!("{}", separator());

    // Ensure symlinks exist for all apps (covers apps pulled from remote
    // whose link_path wasn't symlinked by ingest).
    linker::ensure_links(&shared_config, &local_app_config, &roost_dir);

    println!("{}", separator());

    if git::is_git_repo(&roost_dir) {
        println!("Creating initial commit...");
        let _ = git::auto_commit(&roost_dir, "initial commit");
        if has_remote {
            let branch = crate::git::git_output(&roost_dir, &["rev-parse", "--abbrev-ref", "HEAD"])
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "main".to_string());
            let branch = if branch.is_empty() {
                "main".to_string()
            } else {
                branch
            };
            println!("Pushing to remote...");
            if let Err(e) = git::git(&roost_dir, &["push", "-u", "origin", &branch]) {
                println!("Push failed: {}", e);
                println!("You can push later with `roost sync`.");
            }
        }
    }

    println!("{}", separator());
    println!("Setup complete!");
    println!("Welcome to:");
    println!("{}", logo::LOGO);
    Ok(())
}
