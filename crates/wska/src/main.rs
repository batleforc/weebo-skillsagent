use clap::Parser;
use wska::cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().expect("current directory");
    let env = |k: &str| std::env::var(k).ok();

    let result = match cli.command {
        Command::Install { profile, scope, dry_run } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::install(&root, &profile, scope, dry_run, &cwd, &env).map(|a| {
                println!(
                    "linked {} unlinked {} installers {} skipped {}",
                    a.linked,
                    a.unlinked,
                    a.installers_run.len(),
                    a.skipped.len()
                );
            })
        }
        other => {
            eprintln!("not implemented: {other:?}");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
