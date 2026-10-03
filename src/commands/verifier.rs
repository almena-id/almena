//! `almena verifier`: the tenant's verifiers, and asking a wallet to present
//! credentials as one of them.

use std::thread;

use anyhow::bail;
use clap::Subcommand;
use serde_json::{Value, json};

use crate::commands::described::{self, DescribedCommand, Kind};
use crate::context::Context;
use crate::output::{col, notice};
use crate::wallet;

#[derive(Debug, Subcommand)]
pub enum VerifierCommand {
    #[command(flatten)]
    Described(DescribedCommand),
    /// Ask a wallet, by QR, to present what a form asks for, as this verifier;
    /// waits for the answer and shows the verdict.
    Verify {
        /// The verifier's id (published).
        id: String,
        /// The form whose credentials are asked for.
        #[arg(long, value_name = "FORM_ID")]
        form: String,
    },
}

pub fn run(ctx: &Context, command: VerifierCommand) -> anyhow::Result<()> {
    match command {
        VerifierCommand::Described(command) => described::run(ctx, Kind::Verifier, command),
        VerifierCommand::Verify { id, form } => verify(ctx, &id, &form),
    }
}

fn verify(ctx: &Context, id: &str, form: &str) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    let path = ["tenants", tenant.as_str(), "verifiers", id, "verifications"];
    let opened = api.post(&path, &json!({"form_id": form}))?;
    let (Some(verification), Some(link)) = (opened["id"].as_str(), opened["deep_link"].as_str())
    else {
        bail!("the API's verification is missing its id or link");
    };
    let who = opened["verifier"]["name"].as_str().unwrap_or(id).to_owned();
    wallet::show(link, &format!("present credentials to {who}"));
    let seen = loop {
        let seen: Value = api.get(&[
            "tenants",
            tenant.as_str(),
            "verifiers",
            id,
            "verifications",
            verification,
        ])?;
        match seen["status"].as_str() {
            Some("pending") => thread::sleep(wallet::POLL_EVERY),
            Some("expired") => bail!("the wallet did not answer in time; try again"),
            _ => break seen,
        }
    };
    let result = &seen["result"];
    if ctx.out.is_json() {
        ctx.out.document(result);
        return Ok(());
    }
    ctx.out.list(
        &result["credentials"],
        &[
            col("Credential", "key"),
            col("Verified", "verified"),
            col("Issuer", "issuer"),
            col("Problems", "problems"),
            col("Claims", "claims"),
        ],
    );
    notice(if result["verified"].as_bool() == Some(true) {
        "Verified: every credential the form needs holds."
    } else {
        "Not verified."
    });
    Ok(())
}
