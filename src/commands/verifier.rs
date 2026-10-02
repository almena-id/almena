//! `almena verifier`: the tenant's verifiers.

use crate::commands::described::{self, DescribedCommand, Kind};
use crate::context::Context;

pub type VerifierCommand = DescribedCommand;

pub fn run(ctx: &Context, command: VerifierCommand) -> anyhow::Result<()> {
    described::run(ctx, Kind::Verifier, command)
}
