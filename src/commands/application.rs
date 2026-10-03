//! `almena application`: the applications the tenant's issuers received,
//! deciding on them, and the status of the credentials they were issued.

use std::fs;
use std::path::PathBuf;

use anyhow::Context as _;
use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::context::Context;
use crate::output::{Output, col, notice};
use crate::wallet;

#[derive(Debug, Subcommand)]
pub enum ApplicationCommand {
    /// List the applications received (submitted or decided), newest first.
    List {
        /// Only those sent to this issuer (its id).
        #[arg(long)]
        issuer: Option<String>,
    },
    /// Show one: what the holder signed, its files and presented credentials.
    Get {
        /// Its id.
        id: String,
    },
    /// Save a file the holder uploaded.
    File {
        /// The application's id.
        id: String,
        /// The file field's key.
        key: String,
        /// Where to save it [default: the name it was uploaded with, here].
        #[arg(long, short = 'O', value_name = "PATH")]
        out: Option<PathBuf>,
    },
    /// Accept or reject a submitted application.
    Decide {
        /// Its id.
        id: String,
        /// The decision.
        #[arg(value_enum)]
        decision: Decision,
        /// A note on it, for the holder.
        #[arg(long)]
        note: Option<String>,
    },
    /// Give an issued credential a new status (the issuer's signer signs its status list
    /// from their wallet); revoking is final.
    CredentialStatus {
        /// Its id.
        id: String,
        /// The new status.
        #[arg(value_enum)]
        status: CredentialStatus,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CredentialStatus {
    /// Valid again, after a suspension.
    Valid,
    /// Refused by verifiers until it is valid again.
    Suspended,
    /// Refused by verifiers for good.
    Revoked,
}

impl CredentialStatus {
    fn api(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Suspended => "suspended",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Decision {
    Accept,
    Reject,
}

impl Decision {
    fn api(self) -> &'static str {
        match self {
            Self::Accept => "accepted",
            Self::Reject => "rejected",
        }
    }
}

pub fn run(ctx: &Context, command: ApplicationCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        ApplicationCommand::List { issuer } => {
            let query: Vec<(&str, String)> = issuer
                .into_iter()
                .map(|issuer| ("issuer_id", issuer))
                .collect();
            let received = api.get_query(&["tenants", &tenant, "applications"], &query)?;
            ctx.out.list(
                &received,
                &[
                    col("ID", "id"),
                    col("Status", "status"),
                    col("Issuer", "issuer.name"),
                    col("Credential", "credential_type"),
                    col("Form", "form"),
                    col("Submitted", "submitted_at"),
                    col("Credential status", "credential_status"),
                ],
            );
        }
        ApplicationCommand::Get { id } => {
            show(
                ctx.out,
                &api.get(&["tenants", &tenant, "applications", &id])?,
            );
        }
        ApplicationCommand::File { id, key, out } => {
            let (bytes, name) =
                api.download(&["tenants", &tenant, "applications", &id, "files", &key])?;
            let path = out.unwrap_or_else(|| PathBuf::from(name.unwrap_or(key)));
            fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            notice(format!("Saved {} ({} bytes).", path.display(), bytes.len()));
        }
        ApplicationCommand::Decide { id, decision, note } => {
            let decided = api.post(
                &["tenants", &tenant, "applications", &id, "decision"],
                &json!({"decision": decision.api(), "note": note}),
            )?;
            show(ctx.out, &decided);
            if matches!(decision, Decision::Accept) {
                notice(format!(
                    "Accepted. Issue its credential with `almena issuance get {id}`."
                ));
            }
        }
        ApplicationCommand::CredentialStatus { id, status } => {
            if matches!(status, CredentialStatus::Revoked) {
                ctx.confirm(&format!(
                    "Revoke the credential of application {id}? Verifiers will refuse it for good."
                ))?;
            }
            let mut body = wallet::asking(ctx.locale());
            body["status"] = json!(status.api());
            let request = api.post(
                &["tenants", &tenant, "applications", &id, "credential-status"],
                &body,
            )?;
            wallet::wait(&api, &request, "sign the issuer's status list")?;
            notice(format!("The credential is {} now.", status.api()));
            show(
                ctx.out,
                &api.get(&["tenants", &tenant, "applications", &id])?,
            );
        }
    }
    Ok(())
}

fn show(out: Output, application: &Value) {
    if out.is_json() {
        return out.document(application);
    }
    out.item(
        application,
        &[
            col("ID", "id"),
            col("Status", "status"),
            col("Issuer", "issuer.name"),
            col("Form", "form.name"),
            col("Holder", "holder_did"),
            col("Signature holds", "signature_valid"),
            col("Submitted", "submitted_at"),
            col("Decided", "decided_at"),
            col("Note", "decision_note"),
            col("Issued", "issued_at"),
            col("Valid until", "valid_until"),
            col("Delivered", "delivered_at"),
            col("Credential status", "credential_status"),
            col("Status list", "status_list.uri"),
            col("Entry", "status_list.index"),
        ],
    );
    if let Some(answers) = application["content"]["answers"]
        .as_array()
        .filter(|a| !a.is_empty())
    {
        out.list(
            &Value::Array(answers.clone()),
            &[
                col("Answer", "label"),
                col("Value", "text"),
                col("Verified", "verified"),
            ],
        );
    }
}
