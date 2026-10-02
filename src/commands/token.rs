//! `almena token`: API tokens, for scripts and CI (`ALMENA_TOKEN`).

use clap::Subcommand;
use serde_json::json;

use crate::context::Context;
use crate::output::{col, notice};

#[derive(Debug, Subcommand)]
pub enum TokenCommand {
    /// List your API tokens (never their secrets).
    List,
    /// Make an API token; its secret is printed this once.
    Create {
        /// What it is for (up to 100 characters).
        #[arg(long)]
        name: String,
        /// Days it lasts (1–365).
        #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u16).range(1..=365))]
        days: u16,
    },
    /// Revoke an API token.
    Delete {
        /// The token's id.
        id: String,
    },
}

pub fn run(ctx: &Context, command: TokenCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    match command {
        TokenCommand::List => {
            let tokens = api.get(&["auth", "me", "tokens"])?;
            ctx.out.list(
                &tokens["items"],
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("Created", "created_at"),
                    col("Expires", "expires_at"),
                    col("Last used", "last_used_at"),
                ],
            );
        }
        TokenCommand::Create { name, days } => {
            let made = api.post(
                &["auth", "me", "tokens"],
                &json!({"name": name, "expires_in_days": days}),
            )?;
            if ctx.out.is_json() {
                ctx.out.document(&made);
            } else {
                notice(format!(
                    "API token `{}` made, until {}. Keep it now: it is not shown again.",
                    ctx.out.cell(&made["name"]),
                    ctx.out.cell(&made["expires_at"])
                ));
                println!("{}", ctx.out.cell(&made["token"]));
            }
        }
        TokenCommand::Delete { id } => {
            ctx.confirm("Revoke that token? Whatever uses it stops working.")?;
            api.delete(&["auth", "me", "tokens", &id])?;
            notice("Token revoked.");
        }
    }
    Ok(())
}
