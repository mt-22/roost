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
        app: String,
    },
    Sync,
    Profile(ProfileCmd),
    Diff,
    Log,
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
