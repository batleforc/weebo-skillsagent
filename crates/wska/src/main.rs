mod cli;
mod config;
mod hash;
mod item;
mod ownership;
mod profile;
mod target;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    eprintln!("not implemented: {:?}", cli.command);
    std::process::exit(1);
}
