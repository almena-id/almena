//! `almena signature`: identities whose DID waits for a signature.

use clap::Subcommand;

use crate::context::Context;
use crate::output::{col, notice};

#[derive(Debug, Subcommand)]
pub enum SignatureCommand {
    /// List the identities waiting for a signature, oldest first.
    List,
}

pub fn run(ctx: &Context, command: SignatureCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        SignatureCommand::List => {
            let waiting = api.get(&["tenants", &tenant, "signatures"])?;
            ctx.out.list(
                &waiting,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("Signature", "signature"),
                ],
            );
            if !ctx.out.is_json() && waiting.as_array().is_some_and(|w| !w.is_empty()) {
                notice("Sign each with `almena identity sign <ID>`.");
            }
        }
    }
    Ok(())
}
