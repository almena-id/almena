//! `almena identity`: the tenant's register of DIDs, and signing their logs.

use clap::Subcommand;
use serde_json::{Value, json};

use crate::commands::{PageArgs, paged};
use crate::context::Context;
use crate::output::{Output, col, notice};
use crate::wallet;

#[derive(Debug, Subcommand)]
pub enum IdentityCommand {
    /// List the tenant's identities, newest first.
    List(PageArgs),
    /// Show one: its DID, who uses it, and the document it publishes.
    Get {
        /// Its id.
        id: String,
    },
    /// Register an identity (pending until its first log entry is signed).
    Create {
        /// Its name (1–200 characters).
        #[arg(long)]
        name: String,
    },
    /// Sign its next did:webvh log entry from your wallet.
    Sign {
        /// Its id.
        id: String,
    },
}

pub fn run(ctx: &Context, command: IdentityCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        IdentityCommand::List(page) => {
            let items = paged(&api, &["tenants", &tenant, "identities"], &page)?;
            ctx.out.list(
                &items,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("Used by", "used_by"),
                    col("Signature", "signature"),
                    col("Created", "created_at"),
                ],
            );
        }
        IdentityCommand::Get { id } => {
            show(ctx.out, &api.get(&["tenants", &tenant, "identities", &id])?);
        }
        IdentityCommand::Create { name } => {
            let made = api.post(&["tenants", &tenant, "identities"], &json!({"name": name}))?;
            show(ctx.out, &made);
            notice(format!(
                "Registered. Sign its first log entry with `almena identity sign {}`.",
                made["id"].as_str().unwrap_or_default()
            ));
        }
        IdentityCommand::Sign { id } => {
            let request = api.post(
                &["tenants", &tenant, "identities", &id, "sign"],
                &wallet::asking(ctx.locale()),
            )?;
            wallet::wait(&api, &request, "sign its DID log")?;
            notice("Signed.");
            show(ctx.out, &api.get(&["tenants", &tenant, "identities", &id])?);
        }
    }
    Ok(())
}

fn show(out: Output, identity: &Value) {
    out.item(
        identity,
        &[
            col("ID", "id"),
            col("Name", "name"),
            col("DID", "did"),
            col("Used by", "used_by"),
            col("Signature", "signature"),
            col("Published", "published"),
            col("Document", "document_url"),
            col("Log", "log_url"),
            col("Created", "created_at"),
        ],
    );
}
