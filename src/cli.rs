//! Command-line surface: global options and the subcommand tree.

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

/// The CLI's version: `ALMENA_VERSION` when the build sets it (the release's
/// `year.month.sequence`, see `.github/workflows/release.yml`), else the crate
/// version.
pub const VERSION: &str = match option_env!("ALMENA_VERSION") {
    Some(version) if !version.is_empty() => version,
    _ => env!("CARGO_PKG_VERSION"),
};

#[derive(Debug, Parser)]
#[command(name = "almena", version = VERSION, about, long_about = None, propagate_version = true)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

/// Options accepted by every subcommand.
#[derive(Debug, Args)]
pub struct GlobalArgs {
    /// Print more detail about what the command is doing.
    #[arg(short, long, global = true, env = "ALMENA_VERBOSE")]
    pub verbose: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the shell completion script for `almena`.
    Completions {
        /// Shell to generate the script for.
        shell: Shell,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
