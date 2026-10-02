//! `almena account`: the signed-in account, its alias and its ways in.

use clap::Subcommand;
use serde_json::{Value, json};

use crate::context::{Context, prompt};
use crate::output::{Output, col, notice};
use crate::wallet;

#[derive(Debug, Subcommand)]
pub enum AccountCommand {
    /// Show the account.
    Get,
    /// Change the account's alias.
    Update {
        /// What you like to be called (up to 100 characters).
        #[arg(
            long,
            conflicts_with = "no_alias",
            required_unless_present = "no_alias"
        )]
        alias: Option<String>,
        /// Clear the alias.
        #[arg(long)]
        no_alias: bool,
    },
    /// The account's ways in: its email and linked accounts (wallet, providers).
    WaysIn,
    /// Link an email (a code is sent to it); replaces the one there was.
    LinkEmail {
        /// The address to link.
        email: String,
    },
    /// Unlink the account's email.
    UnlinkEmail,
    /// Link your Almena wallet: it signs in, and signs for the tenant.
    LinkWallet,
    /// Unlink a linked account (wallet or provider), by its id in `ways-in`.
    Unlink {
        /// The linked account's id.
        id: String,
    },
}

pub fn run(ctx: &Context, command: AccountCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    match command {
        AccountCommand::Get => show(ctx.out, &api.get(&["auth", "me"])?),
        AccountCommand::Update { alias, .. } => {
            let account = api.patch(
                &["auth", "me"],
                &json!({"alias": alias.unwrap_or_default()}),
            )?;
            show(ctx.out, &account);
        }
        AccountCommand::WaysIn => {
            let ways = api.get(&["auth", "me", "ways-in"])?;
            if ctx.out.is_json() {
                ctx.out.document(&ways);
                return Ok(());
            }
            notice(format!("Email: {}", ctx.out.cell(&ways["email"])));
            ctx.out.list(
                &ways["accounts"],
                &[
                    col("ID", "id"),
                    col("Provider", "provider"),
                    col("Email", "email"),
                    col("DID", "did"),
                    col("Linked", "created_at"),
                ],
            );
        }
        AccountCommand::LinkEmail { email } => {
            api.post(
                &["auth", "code"],
                &json!({"email": email, "locale": ctx.locale()}),
            )?;
            notice(format!("A six-digit code is on its way to {email}."));
            let code = prompt("Code: ")?;
            let result = api.post(
                &["auth", "me", "email"],
                &json!({"email": email, "code": code}),
            )?;
            linked(ctx.out, &result, "That email");
        }
        AccountCommand::UnlinkEmail => {
            ctx.confirm("Unlink the account's email?")?;
            api.delete(&["auth", "me", "email"])?;
            notice("Email unlinked.");
        }
        AccountCommand::LinkWallet => {
            let request = api.post(
                &["auth", "wallet", "requests"],
                &json!({"purpose": "link", "locale": ctx.locale(), "client": wallet::CLIENT}),
            )?;
            let result = wallet::wait(&api, &request, "link it to your account")?;
            linked(ctx.out, &result, "That wallet");
        }
        AccountCommand::Unlink { id } => {
            ctx.confirm("Unlink that account?")?;
            api.delete(&["auth", "me", "accounts", &id])?;
            notice("Unlinked.");
        }
    }
    Ok(())
}

fn show(out: Output, account: &Value) {
    out.item(
        account,
        &[
            col("ID", "id"),
            col("Email", "email"),
            col("Alias", "alias"),
            col("Created", "created_at"),
        ],
    );
}

/// `linked`, or `taken` by another account (which the portal can merge).
fn linked(out: Output, result: &Value, what: &str) {
    if out.is_json() {
        return out.document(result);
    }
    if result["status"] == "taken" {
        notice(format!(
            "{what} already belongs to another account. To move into it, sign in to the \
             registry portal: it offers the move while this account is still empty."
        ));
    } else {
        notice("Linked.");
    }
}
