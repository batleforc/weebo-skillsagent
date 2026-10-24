use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "wska",
    version,
    about = "Link this repo's skills into a Claude Code, Gemini CLI or Antigravity target"
)]
pub struct Cli {
    /// Which agent's directory to link into. Gemini CLI and Antigravity get skills only:
    /// agents, MCP servers and installers are Claude Code formats
    #[arg(long, global = true, value_enum, default_value_t = Agent::Claude)]
    pub agent: Agent,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum HudCommand {
    /// Print the powerline status line for Claude Code's `statusLine` (JSON on stdin)
    Statusline,
    /// Set Claude Code's `statusLine` to `wska hud statusline` (no jq needed)
    Install {
        /// Settings file to edit (default: $CLAUDE_CONFIG_DIR/settings.json, else ~/.claude/settings.json)
        #[arg(long)]
        settings: Option<PathBuf>,
    },
    /// Remove `statusLine` when it is `wska hud statusline`
    Uninstall {
        #[arg(long)]
        settings: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// The weebo-hud status line
    Hud {
        #[command(subcommand)]
        cmd: HudCommand,
    },
    /// Link the items named by the given profiles, removing owned links not in that set
    /// With neither --profile nor --item, re-applies the selection recorded in the target's wska.toml
    Install {
        #[arg(long)]
        profile: Vec<String>,
        /// A single item to link besides the profiles' (repeatable)
        #[arg(long)]
        item: Vec<String>,
        /// An extra items repo (git URL) whose items and profiles join this one's,
        /// recorded in wska.toml (repeatable)
        #[arg(long)]
        repo: Vec<String>,
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
        /// An extra items repo (git URL) whose items and profiles join this one's,
        /// recorded in wska.toml (repeatable)
        #[arg(long)]
        repo: Vec<String>,
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
        #[arg(long)]
        dry_run: bool,
        /// Actually execute installer items (bmad, graphify, ...); otherwise they are skipped
        #[arg(long)]
        run_installers: bool,
    },
    /// Remove profiles or items from the target's wska.toml selection, then install what is left
    Remove {
        #[arg(long)]
        profile: Vec<String>,
        /// A single item recorded with --item (repeatable)
        #[arg(long)]
        item: Vec<String>,
        /// An extra items repo recorded with --repo; its items are unlinked (repeatable)
        #[arg(long)]
        repo: Vec<String>,
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
    /// Table of items and the profiles that list them, profiles with --profiles,
    /// or the target's wska.toml selection with --recorded
    List {
        #[arg(long, conflicts_with = "recorded")]
        profiles: bool,
        #[arg(long)]
        recorded: bool,
        /// Target whose installed state (and --recorded selection) is shown
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
    },
    /// Check whether each item can actually work here: declared tools on PATH,
    /// and a usable SKILL.md. Exits non-zero if anything is unhealthy.
    Doctor {
        /// Target whose wska.toml names the extra items repos to check too
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Agent {
    Claude,
    Gemini,
    Antigravity,
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
                ..
            } => {
                assert_eq!(profile, vec!["base".to_string(), "frontend".to_string()]);
                assert_eq!(item, vec!["weebo-techdoc".to_string()]);
                assert_eq!(scope, Scope::User);
                assert!(!dry_run);
                assert!(!run_installers);
            }
            other => panic!("expected Install, got {other:?}"),
        }
        assert_eq!(cli.agent, Agent::Claude);
    }

    #[test]
    fn agent_is_accepted_after_the_subcommand() {
        let cli = Cli::parse_from(["wska", "status", "--agent", "antigravity"]);
        assert_eq!(cli.agent, Agent::Antigravity);
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
