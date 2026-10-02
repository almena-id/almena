//! `almena issuance`: issuing an accepted application's credential — settle
//! its claims and validity, then its issuer's signer signs it from their wallet.

use std::path::PathBuf;

use anyhow::bail;
use clap::Subcommand;
use serde_json::{Map, Value, json};

use crate::commands::read_json;
use crate::context::Context;
use crate::output::{Output, col, notice};
use crate::wallet;

#[derive(Debug, Subcommand)]
pub enum IssuanceCommand {
    /// The credential to issue: its claims, proposed from the application.
    Get {
        /// The application's id.
        application: String,
    },
    /// Settle the claims and validity (the issuer's signer); unset ones keep what is proposed.
    Set {
        /// The application's id.
        application: String,
        /// The claims as a JSON object (`-` for stdin).
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// One claim, CLAIM=VALUE (VALUE read as JSON when it is, else as text); repeat for more.
        #[arg(long, value_name = "CLAIM=VALUE", value_parser = parse_claim)]
        claim: Vec<(String, Value)>,
        /// The last day it is valid (YYYY-MM-DD, after today).
        #[arg(long, value_name = "DATE")]
        valid_until: Option<String>,
    },
    /// Sign the settled credential from your wallet: the application is then issued.
    Sign {
        /// The application's id.
        application: String,
    },
}

fn parse_claim(raw: &str) -> Result<(String, Value), String> {
    let (claim, value) = raw
        .split_once('=')
        .ok_or_else(|| format!("expected CLAIM=VALUE, got `{raw}`"))?;
    let value = serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()));
    Ok((claim.trim().to_owned(), value))
}

pub fn run(ctx: &Context, command: IssuanceCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        IssuanceCommand::Get { application } => {
            show(
                ctx.out,
                &api.get(&["tenants", &tenant, "applications", &application, "issuance"])?,
            );
        }
        IssuanceCommand::Set {
            application,
            file,
            claim,
            valid_until,
        } => {
            let path = [
                "tenants",
                tenant.as_str(),
                "applications",
                application.as_str(),
                "issuance",
            ];
            let proposal = api.get(&path)?;
            let mut claims: Map<String, Value> = proposal["claims"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter(|c| !c["value"].is_null())
                .filter_map(|c| Some((c["field"]["id"].as_str()?.to_owned(), c["value"].clone())))
                .collect();
            if let Some(file) = file {
                match read_json(&file)? {
                    Value::Object(given) => claims.extend(given),
                    _ => bail!("the claims file must hold a JSON object"),
                }
            }
            claims.extend(claim);
            let valid_until = valid_until.unwrap_or_else(|| {
                proposal["valid_until"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            });
            let draft = api.put(
                &path,
                &json!({"claims": claims, "valid_until": valid_until}),
            )?;
            if ctx.out.is_json() {
                ctx.out.document(&draft);
            } else {
                notice(format!(
                    "Settled, valid until {}.",
                    ctx.out.cell(&draft["valid_until"])
                ));
                let rows: Vec<Value> = draft["claims"]
                    .as_object()
                    .map(|claims| {
                        claims
                            .iter()
                            .map(|(k, v)| json!({"claim": k, "value": v}))
                            .collect()
                    })
                    .unwrap_or_default();
                ctx.out.list(
                    &Value::Array(rows),
                    &[col("Claim", "claim"), col("Value", "value")],
                );
                notice(format!(
                    "Sign it with `almena issuance sign {application}`."
                ));
            }
        }
        IssuanceCommand::Sign { application } => {
            let request = api.post(
                &[
                    "tenants",
                    &tenant,
                    "applications",
                    &application,
                    "issuance",
                    "sign",
                ],
                &wallet::asking(ctx.locale()),
            )?;
            wallet::wait(&api, &request, "sign the credential")?;
            notice("Issued: the holder's wallet can take it now.");
            let issued = api.get(&["tenants", &tenant, "applications", &application])?;
            ctx.out.item(
                &issued,
                &[
                    col("ID", "id"),
                    col("Status", "status"),
                    col("Issued", "issued_at"),
                    col("Valid until", "valid_until"),
                ],
            );
        }
    }
    Ok(())
}

fn show(out: Output, proposal: &Value) {
    if out.is_json() {
        return out.document(proposal);
    }
    out.item(
        proposal,
        &[
            col("Credential", "credential_type.id"),
            col("Holder", "holder_did"),
            col("Valid until", "valid_until"),
            col("You can sign", "can_sign"),
            col("Needs a signer", "signer_needed"),
        ],
    );
    out.list(
        &proposal["claims"],
        &[
            col("Claim", "field.id"),
            col("Label", "field.labels"),
            col("Required", "required"),
            col("Value", "value"),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_are_json_or_text() {
        assert_eq!(parse_claim("age=42"), Ok(("age".into(), json!(42))));
        assert_eq!(parse_claim("name=Ada"), Ok(("name".into(), json!("Ada"))));
        assert_eq!(
            parse_claim(r#"tags=["a"]"#),
            Ok(("tags".into(), json!(["a"])))
        );
        assert!(parse_claim("nothing").is_err());
    }
}
