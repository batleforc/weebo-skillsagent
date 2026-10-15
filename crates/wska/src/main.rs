use clap::Parser;
use wska::cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().expect("current directory");
    let env = |k: &str| std::env::var(k).ok();

    let result = match cli.command {
        Command::Install {
            profile,
            scope,
            dry_run,
            run_installers,
        } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::install(&root, &profile, scope, dry_run, run_installers, &cwd, &env).map(
                |a| {
                    println!(
                        "linked {} unlinked {} installers {} skipped {}",
                        a.linked,
                        a.unlinked,
                        a.installers_run.len(),
                        a.skipped.len()
                    );
                },
            )
        }
        Command::Status { scope } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::status(&root, scope, &cwd, &env).map(|rows| {
                for row in rows {
                    println!("{:<28} {:?}", row.name, row.state);
                }
            })
        }
        Command::List { profiles } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::list(&root, profiles).map(|names| {
                for name in names {
                    println!("{name}");
                }
            })
        }
        Command::Doctor => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::doctor(&root, &env).map(|rows| {
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
            })
        }
        Command::Sync { source, check } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::sync(&root, &source, check).map(|outcomes| {
                let mut bad = false;
                for o in &outcomes {
                    println!("{o:?}");
                    if matches!(o, wska::sync::Outcome::MissingLicence { .. }) {
                        bad = true;
                    }
                }
                if check && bad {
                    std::process::exit(1);
                }
            })
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
