use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "wska",
    version,
    about = "Link this repo's skills into a Claude Code target"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Link the items named by the given profiles, removing owned links not in that set
    /// With neither --profile nor --item, re-applies the selection recorded in the target's wska.toml
    Install {
        #[arg(long)]
        profile: Vec<String>,
        /// A single item to link besides the profiles' (repeatable)
        #[arg(long)]
        item: Vec<String>,
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
        #[arg(long)]
        dry_run: bool,
        /// Actually execute installer items (bmad, graphify, ...); otherwise they are skipped
        #[arg(long)]
        run_installers: bool,
    },
    /// Add profiles or items to the target's wska.toml selection, then install the whole of it
    Add {
        #[arg(long)]
        profile: Vec<String>,
        /// A single item to link besides the profiles' (repeatable)
        #[arg(long)]
        item: Vec<String>,
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
        #[arg(long)]
        dry_run: bool,
        /// Actually execute installer items (bmad, graphify, ...); otherwise they are skipped
        #[arg(long)]
        run_installers: bool,
    },
    /// Pull the items repo (`git pull --ff-only`), then re-install the given profiles, if any
    /// With neither --profile nor --item, re-installs the target's wska.toml if there is one
    Update {
        #[arg(long)]
        profile: Vec<String>,
        /// A single item to link besides the profiles' (repeatable)
        #[arg(long)]
        item: Vec<String>,
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
        /// Actually execute installer items when re-installing
        #[arg(long)]
        run_installers: bool,
    },
    /// Report what is linked in a target
    Status {
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
    },
    /// List items in this repo, or profiles with --profiles
    List {
        #[arg(long)]
        profiles: bool,
    },
    /// Check whether each item can actually work here: declared tools on PATH,
    /// and a usable SKILL.md. Exits non-zero if anything is unhealthy.
    Doctor,
    /// Refresh vendored sources from upstream
    Sync {
        source: Vec<String>,
        #[arg(long)]
        check: bool,
    },
    /// Print a shell completion script, e.g. `wska completions bash > ~/.local/share/bash-completion/completions/wska`
    Completions { shell: clap_complete::Shell },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    User,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_install_with_two_profiles_and_scope() {
        let cli = Cli::parse_from([
            "wska",
            "install",
            "--profile",
            "base",
            "--profile",
            "frontend",
            "--item",
            "weebo-techdoc",
            "--scope",
            "user",
        ]);
        match cli.command {
            Command::Install {
                profile,
                item,
                scope,
                dry_run,
                run_installers,
            } => {
                assert_eq!(profile, vec!["base".to_string(), "frontend".to_string()]);
                assert_eq!(item, vec!["weebo-techdoc".to_string()]);
                assert_eq!(scope, Scope::User);
                assert!(!dry_run);
                assert!(!run_installers);
            }
            other => panic!("expected Install, got {other:?}"),
        }
    }

    #[test]
    fn scope_defaults_to_project() {
        let cli = Cli::parse_from(["wska", "status"]);
        match cli.command {
            Command::Status { scope } => assert_eq!(scope, Scope::Project),
            other => panic!("expected Status, got {other:?}"),
        }
    }
}
