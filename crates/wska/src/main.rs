use clap::Parser;
use wska::cli::{Cli, Command, HudCommand};

/// Cloned on first run when wska is used outside this repo without WSKA_REPO.
const REPO_URL: &str = "https://github.com/batleforc/weebo-skillsagent";

fn configured_repository(
    command: &wska::cli::Command,
    agent: wska::cli::Agent,
    cwd: &std::path::Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let scope = match command {
        wska::cli::Command::Install { scope, .. }
        | wska::cli::Command::Add { scope, .. }
        | wska::cli::Command::Remove { scope, .. }
        | wska::cli::Command::Update { scope, .. }
        | wska::cli::Command::Status { scope }
        | wska::cli::Command::Doctor { scope }
        | wska::cli::Command::List {
            scope,
            recorded: true,
            ..
        } => *scope,
        _ => return None,
    };
    let target = wska::target::resolve(scope, agent, cwd, env).ok()?;
    wska::selection::Selection::load(&target).ok()??.repository
}

fn configured_agents(
    command: &wska::cli::Command,
    agent: wska::cli::Agent,
    cwd: &std::path::Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> (Vec<wska::cli::Agent>, Option<wska::selection::Selection>) {
    let scope = match command {
        wska::cli::Command::Install { scope, .. } => *scope,
        _ => return (vec![agent], None),
    };
    let target = match wska::target::resolve(scope, agent, cwd, env) {
        Ok(target) => target,
        Err(_) => return (vec![agent], None),
    };
    let recorded = wska::selection::Selection::load(&target).ok().flatten();
    let agents = recorded
        .as_ref()
        .filter(|selection| !selection.agents.is_empty())
        .map(|selection| selection.agents.clone())
        .unwrap_or_else(|| vec![agent]);
    (agents, recorded)
}

fn print_applied(a: &wska::apply::Applied) {
    println!(
        "linked {} unlinked {} installers {} skipped {}",
        a.linked,
        a.unlinked,
        a.installers_run.len(),
        a.skipped.len()
    );
}

// Rust ignores SIGPIPE, so `wska list | head` made every print panic on the
// closed pipe. Restore the default: the process just exits, like any CLI.
#[cfg(unix)]
#[expect(
    unsafe_code,
    reason = "libc signal() has no std equivalent; no thread exists yet"
)]
fn default_sigpipe() {
    const SIGPIPE: i32 = 13; // same on Linux and macOS
    const SIG_DFL: usize = 0;
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    // SAFETY: called first thing in main, before any thread exists.
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

fn main() {
    #[cfg(unix)]
    default_sigpipe();
    let cli = Cli::parse();
    // Before items_root: completions must work anywhere, without cloning the repo.
    if let Command::Completions { shell } = cli.command {
        use clap::CommandFactory;
        clap_complete::generate(shell, &mut Cli::command(), "wska", &mut std::io::stdout());
        return;
    }
    // No repo needed, and nothing may reach stderr or fail the status line.
    if let Command::Hud { cmd } = cli.command {
        let res = match cmd {
            HudCommand::Statusline => {
                wska::hud::statusline();
                return;
            }
            HudCommand::Install { settings } => wska::hud::install::run(true, settings),
            HudCommand::Uninstall { settings } => wska::hud::install::run(false, settings),
        };
        match res {
            Ok(msg) => println!("{msg}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Command::Mcp { cmd } = &cli.command {
        let wska::cli::McpCommand::Setup { scope } = cmd;
        let cwd = match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(e) => {
                eprintln!("error: reading the current directory: {e}");
                std::process::exit(1);
            }
        };
        let env = |k: &str| std::env::var(k).ok();
        match wska::mcp::setup(cli.agent, *scope, &cwd, &env) {
            Ok(message) => println!("{message}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("error: reading the current directory: {e}");
            std::process::exit(1);
        }
    };
    let env = |k: &str| std::env::var(k).ok();
    let configured = configured_repository(&cli.command, cli.agent, &cwd, &env);
    let root = match wska::target::items_root(&cwd, &env, REPO_URL, configured.as_deref()) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let agent = cli.agent;
    let result = match cli.command {
        Command::Install {
            profile,
            item,
            repo,
            scope,
            dry_run,
            run_installers,
        } => {
            let requested = wska::selection::Selection {
                repository: None,
                agents: vec![],
                profiles: profile,
                items: item,
                repos: repo,
            };
            let (agents, recorded) = configured_agents(
                &Command::Install {
                    profile: requested.profiles.clone(),
                    item: requested.items.clone(),
                    repo: requested.repos.clone(),
                    scope,
                    dry_run,
                    run_installers,
                },
                agent,
                &cwd,
                &env,
            );
            let selection = if requested.is_empty() {
                recorded.unwrap_or_default()
            } else {
                requested
            };
            let mut result: Result<(), wska::app::AppError> = Ok(());
            for target_agent in agents {
                if result.is_ok() {
                    result = wska::app::install(
                        &root,
                        &selection,
                        scope,
                        target_agent,
                        dry_run,
                        run_installers,
                        &cwd,
                        &env,
                    )
                    .map(|a| print_applied(&a));
                }
            }
            result
        }
        Command::Add {
            profile,
            item,
            repo,
            scope,
            dry_run,
            run_installers,
        } => {
            let extra = wska::selection::Selection {
                repository: None,
                agents: vec![],
                profiles: profile,
                items: item,
                repos: repo,
            };
            wska::app::add(
                &root,
                &extra,
                scope,
                agent,
                dry_run,
                run_installers,
                &cwd,
                &env,
            )
            .map(|a| print_applied(&a))
        }
        Command::Remove {
            profile,
            item,
            repo,
            scope,
            dry_run,
            run_installers,
        } => {
            let gone = wska::selection::Selection {
                repository: None,
                agents: vec![],
                profiles: profile,
                items: item,
                repos: repo,
            };
            wska::app::remove(
                &root,
                &gone,
                scope,
                agent,
                dry_run,
                run_installers,
                &cwd,
                &env,
            )
            .map(|a| print_applied(&a))
        }
        Command::Update {
            profile,
            item,
            scope,
            run_installers,
        } => {
            let selection = wska::selection::Selection {
                profiles: profile,
                items: item,
                agents: vec![],
                ..Default::default()
            };
            wska::app::update(&root, &selection, scope, agent, run_installers, &cwd, &env).map(
                |result| {
                    println!("updated {}", root.display());
                    for (repo, pull) in result.pulls {
                        println!(
                            "pulled {}: source {} -> destination {}",
                            repo.display(),
                            pull.source,
                            pull.destination
                        );
                    }
                    if let Some(a) = result.applied {
                        print_applied(&a);
                    }
                },
            )
        }
        Command::Status { scope } => {
            wska::app::status(&root, scope, agent, &cwd, &env).map(|rows| {
                for row in rows {
                    println!("{:<28} {:?}", row.name, row.state);
                }
            })
        }
        Command::List {
            profiles,
            recorded,
            scope,
        } => if recorded {
            wska::app::list_recorded(&root, scope, agent, &cwd, &env)
        } else {
            wska::app::list(&root, profiles, scope, agent, &cwd, &env)
        }
        .map(|table| print!("{table}")),
        Command::Doctor { scope } => {
            wska::app::doctor(&root, scope, agent, &cwd, &env).map(|rows| {
                let mut unhealthy: usize = 0;
                for row in &rows {
                    match &row.health {
                        wska::doctor::Health::Ok => println!("{:<28} ok", row.name),
                        wska::doctor::Health::Missing(tools) => {
                            unhealthy = unhealthy.saturating_add(1);
                            println!("{:<28} missing: {}", row.name, tools.join(", "));
                        }
                        wska::doctor::Health::Invalid(reason) => {
                            unhealthy = unhealthy.saturating_add(1);
                            println!("{:<28} invalid: {reason}", row.name);
                        }
                    }
                }
                if unhealthy > 0 {
                    eprintln!("{unhealthy} item(s) cannot work here");
                    std::process::exit(1);
                }
            })
        }
        Command::Sync { source, check } => wska::app::sync(&root, &source, check).map(|outcomes| {
            for outcome in &outcomes {
                println!("{outcome:?}");
            }
            if !check {
                return;
            }
            let verdict = wska::sync::check_verdict(&outcomes);
            match &verdict {
                wska::sync::CheckVerdict::Clean => {}
                wska::sync::CheckVerdict::Drift(skills) => eprintln!(
                    "{} skill(s) behind upstream: {}. Run `task skills:sync`, \
                         then read `git diff skills/` before committing.",
                    skills.len(),
                    skills.join(", ")
                ),
                wska::sync::CheckVerdict::LicenceMissing(sources) => eprintln!(
                    "no licence recorded for: {}. Add one to vendor.toml.",
                    sources.join(", ")
                ),
            }
            let code = verdict.exit_code();
            if code != 0 {
                std::process::exit(code);
            }
        }),
        #[expect(
            clippy::unreachable,
            reason = "Completions and Hud return before items_root, above"
        )]
        Command::Completions { .. } | Command::Hud { .. } | Command::Mcp { .. } => {
            unreachable!("handled above")
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
