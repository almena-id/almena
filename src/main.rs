mod cli;
mod client;
mod commands;
mod config;
mod context;
mod credentials;
mod output;
mod wallet;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::client::ApiError;
use crate::context::NotSignedIn;

/// Exit statuses (2, a usage error, is clap's own).
const FAILED: u8 = 1;
const UNAUTHORIZED: u8 = 3;
const NOT_FOUND: u8 = 4;
const REFUSED: u8 = 5;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match commands::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(exit_status(&error))
        }
    }
}

/// What a failure exits with: scripts tell apart signing in again, a missing
/// item and a refused request without reading the message.
fn exit_status(error: &anyhow::Error) -> u8 {
    if error.downcast_ref::<NotSignedIn>().is_some() {
        return UNAUTHORIZED;
    }
    match error.downcast_ref::<ApiError>().map(|e| e.status) {
        Some(401 | 403) => UNAUTHORIZED,
        Some(404) => NOT_FOUND,
        Some(400 | 409 | 410 | 422 | 429) => REFUSED,
        _ => FAILED,
    }
}
