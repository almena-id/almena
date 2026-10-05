//! `almena pending`: what waits in the tenant to be signed or published.

use clap::Subcommand;

use crate::context::Context;
use crate::output::{col, notice};

#[derive(Debug, Subcommand)]
pub enum PendingCommand {
    /// List what waits to be signed or published, in the order it is done:
    /// identities, then issuers, verifiers and mediators, then issuers'
    /// status lists and credentials.
    List,
}

pub fn run(ctx: &Context, command: PendingCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        PendingCommand::List => {
            let pending = api.get(&["tenants", &tenant, "pending"])?;
            ctx.out.list(
                &pending,
                &[
                    col("Kind", "kind"),
                    col("ID", "id"),
                    col("Name", "name"),
                    col("Issuer", "issuer.name"),
                    col("State", "state"),
                    col("To do", "action"),
                    col("First", "blocked_by"),
                    col("Yours", "yours"),
                ],
            );
            if !ctx.out.is_json() && pending.as_array().is_some_and(|p| !p.is_empty()) {
                notice(
                    "Sign an identity with `almena identity sign <ID>`, publish with \
                     `almena <kind> publish <ID>`, sign a status list with \
                     `almena issuer status-list sign <ISSUER> --status-list <ID>` and \
                     issue with `almena issuance update|sign <ID>`.",
                );
            }
        }
    }
    Ok(())
}
