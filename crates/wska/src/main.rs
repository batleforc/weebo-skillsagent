use clap::Parser;
use wska::cli::{Cli, Command};

/// Cloned on first run when wska is used outside this repo without WSKA_REPO.
const REPO_URL: &str = "https://github.com/batleforc/weebo-skillsagent";

fn main() {
    let cli = Cli::parse();
    // Before items_root: completions must work anywhere, without cloning the repo.
    if let Command::Completions { shell } = cli.command {
        use clap::CommandFactory;
        clap_complete::generate(shell, &mut Cli::command(), "wska", &mut std::io::stdout());
        return;
    }
    let cwd = std::env::current_dir().expect("current directory");
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
            scope,
            dry_run,
            run_installers,
        } => wska::app::install(&root, &profile, scope, dry_run, run_installers, &cwd, &env).map(
            |a| {
                println!(
                    "linked {} unlinked {} installers {} skipped {}",
                    a.linked,
                    a.unlinked,
                    a.installers_run.len(),
                    a.skipped.len()
                );
            },
        ),
        Command::Status { scope } => wska::app::status(&root, scope, &cwd, &env).map(|rows| {
            for row in rows {
                println!("{:<28} {:?}", row.name, row.state);
            }
        }),
        Command::List { profiles } => wska::app::list(&root, profiles).map(|names| {
            for name in names {
                println!("{name}");
            }
        }),
        Command::Doctor => wska::app::doctor(&root, &env).map(|rows| {
            let mut unhealthy = 0;
            for row in &rows {
                match &row.health {
                    wska::doctor::Health::Ok => println!("{:<28} ok", row.name),
                    wska::doctor::Health::Missing(tools) => {
                        unhealthy += 1;
                        println!("{:<28} missing: {}", row.name, tools.join(", "));
                    }
                    wska::doctor::Health::Invalid(reason) => {
                        unhealthy += 1;
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
        Command::Completions { .. } => unreachable!("handled above"),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
