mod cli;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    eprintln!("not implemented: {:?}", cli.command);
    std::process::exit(1);
}
