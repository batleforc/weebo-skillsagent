use clap::Parser;
use wska::cli::{Cli, Command};

/// Cloned on first run when wska is used outside this repo without WSKA_REPO.
const REPO_URL: &str = "https://github.com/batleforc/weebo-skillsagent";

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
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("error: reading the current directory: {e}");
            std::process::exit(1);
        }
    };
    let env = |k: &str| std::env::var(k).ok();
    let root = match wska::target::items_root(&cwd, &env, REPO_URL) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let result = match cli.command {
        Command::Install {
            profile,
            item,
            scope,
            dry_run,
            run_installers,
        } => {
            let selection = wska::selection::Selection {
                profiles: profile,
                items: item,
            };
            wska::app::install(
                &root,
                &selection,
                scope,
                dry_run,
                run_installers,
                &cwd,
                &env,
            )
            .map(|a| print_applied(&a))
        }
        Command::Add {
            profile,
            item,
            scope,
            dry_run,
            run_installers,
        } => {
            let extra = wska::selection::Selection {
                profiles: profile,
                items: item,
            };
            wska::app::add(&root, &extra, scope, dry_run, run_installers, &cwd, &env)
                .map(|a| print_applied(&a))
        }
        Command::Remove {
            profile,
            item,
            scope,
            dry_run,
            run_installers,
        } => {
            let gone = wska::selection::Selection {
                profiles: profile,
                items: item,
            };
            wska::app::remove(&root, &gone, scope, dry_run, run_installers, &cwd, &env)
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
            };
            wska::app::update(&root, &selection, scope, run_installers, &cwd, &env).map(|applied| {
                println!("updated {}", root.display());
                if let Some(a) = applied {
                    print_applied(&a);
                }
            })
        }
        Command::Status { scope } => wska::app::status(&root, scope, &cwd, &env).map(|rows| {
            for row in rows {
                println!("{:<28} {:?}", row.name, row.state);
            }
        }),
        Command::List {
            profiles,
            recorded,
            scope,
        } => if recorded {
            wska::app::list_recorded(&root, scope, &cwd, &env)
        } else {
            wska::app::list(&root, profiles, scope, &cwd, &env)
        }
        .map(|table| print!("{table}")),
        Command::Doctor => wska::app::doctor(&root, &env).map(|rows| {
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
        }),
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
            reason = "Completions returns before items_root, above"
        )]
        Command::Completions { .. } => unreachable!("handled above"),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
