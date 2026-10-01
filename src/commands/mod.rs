//! One module per subcommand; `run` dispatches the parsed command line.

mod completions;

use crate::cli::{Cli, Command};

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Completions { shell } => completions::run(shell),
    }
}
