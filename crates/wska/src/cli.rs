use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "wska", about = "Link this repo's skills into a Claude Code target")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Link the items named by the given profiles, removing owned links not in that set
    Install {
        #[arg(long, required = true)]
        profile: Vec<String>,
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
        #[arg(long)]
        dry_run: bool,
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
    /// Refresh vendored sources from upstream
    Sync {
        source: Vec<String>,
        #[arg(long)]
        check: bool,
    },
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
            "wska", "install", "--profile", "base", "--profile", "frontend", "--scope", "user",
        ]);
        match cli.command {
            Command::Install { profile, scope, dry_run } => {
                assert_eq!(profile, vec!["base".to_string(), "frontend".to_string()]);
                assert_eq!(scope, Scope::User);
                assert!(!dry_run);
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
