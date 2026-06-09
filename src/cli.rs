use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "roost", version = "0.2.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Add {
        path: PathBuf,
    },
    Remove {
        app: Option<String>,
        #[arg(long)]
        all: bool,
    },
    Sync,
    Profile(ProfileCmd),
    Diff,
    Log,
    Ignore {
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        list: bool,
        pattern: Option<String>,
    },
    Undo {
        n: Option<usize>,
    },
    Rollback {
        hash: String,
    },
    Restore {
        app: String,
    },
    Remote {
        url: Option<String>,
    },
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    Adopt,
    Where {
        app: String,
        #[arg(long)]
        profile: Option<String>,
    },
    List {
        #[arg(long)]
        profile: Option<String>,
    },
    Save {
        message: Option<String>,
    },
    Status,
    /// Import an app from another profile via symlink (zero-copy).
    ///
    /// The app will appear in both profiles but files stay in the source profile.
    Import {
        app: String,
        #[arg(long)]
        from: String,
    },
    /// Copy an app from the active profile to another profile (physical copy).
    ///
    /// Creates an independent copy of the app's files in the target profile.
    Copy {
        app: String,
        #[arg(long)]
        to: String,
    },
    /// Generate shell completion scripts for supported shells.
    ///
    /// Prints a completion script to stdout for the given shell.
    /// Source it dynamically in your shell config, or redirect to a file.
    ///
    /// Supported shells: bash, zsh, fish, powershell, elvish.
    ///
    /// Examples:
    ///   eval "$(roost completions bash)"
    ///   roost completions zsh > ~/.zsh/completions/_roost
    ///   roost completions fish > ~/.config/fish/completions/roost.fish
    Completions {
        shell: String,
    },
}

#[derive(Args)]
pub struct ProfileCmd {
    #[command(subcommand)]
    pub action: ProfileAction,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    List,
    Switch {
        name: String,
    },
    Add {
        name: String,
        #[arg(long)]
        from: Option<String>,
    },
    Delete {
        name: String,
    },
    Rename {
        old: String,
        new: String,
    },
}
