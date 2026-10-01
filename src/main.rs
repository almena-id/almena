mod cli;
mod commands;

use clap::Parser;

use crate::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    commands::run(cli)
}
