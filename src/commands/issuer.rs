//! `almena issuer`: the tenant's issuers, the credential types they grant, and
//! their status lists.

use clap::Subcommand;
use serde_json::{Map, Value, json};

use crate::commands::described::{self, DescribedCommand, Kind};
use crate::context::Context;
use crate::output::{Output, col, notice};
use crate::wallet;

#[derive(Debug, Subcommand)]
pub enum IssuerCommand {
    #[command(flatten)]
    Described(DescribedCommand),
    /// The credential types it grants, and the form each is applied for with.
    #[command(subcommand)]
    CredentialTypes(CredentialTypesCommand),
    /// Its status lists: where verifiers check whether its credentials still hold.
    #[command(subcommand)]
    StatusList(StatusListCommand),
}

#[derive(Debug, Subcommand)]
pub enum StatusListCommand {
    /// List its status lists: entries taken, revoked, suspended, and whether each must be signed.
    List {
        /// The issuer's id.
        id: String,
    },
    /// Sign a status list as it is from your wallet (the issuer's signer): before its first
    /// credential, or after the key it was signed with left the issuer's DID.
    Sign {
        /// The issuer's id.
        id: String,
        /// The list to sign [default: its current one, made if it has none].
        #[arg(long, value_name = "ID")]
        status_list: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum CredentialTypesCommand {
    /// Show the types it grants, and their forms.
    Get {
        /// The issuer's id.
        id: String,
    },
    /// Declare the types it grants (replaces the list).
    Set {
        /// The issuer's id.
        id: String,
        /// A credential type it grants (see `almena catalog credentials`); repeat for more.
        #[arg(long = "type", value_name = "TYPE")]
        types: Vec<String>,
        /// TYPE=FORM_ID: the form a type is applied for with, which makes it an offer.
        #[arg(long = "form", value_name = "TYPE=FORM_ID", value_parser = parse_form)]
        forms: Vec<(String, String)>,
    },
}

fn parse_form(raw: &str) -> Result<(String, String), String> {
    raw.split_once('=')
        .map(|(kind, form)| (kind.trim().to_owned(), form.trim().to_owned()))
        .filter(|(kind, form)| !kind.is_empty() && !form.is_empty())
        .ok_or_else(|| format!("expected TYPE=FORM_ID, got `{raw}`"))
}

pub fn run(ctx: &Context, command: IssuerCommand) -> anyhow::Result<()> {
    match command {
        IssuerCommand::Described(command) => described::run(ctx, Kind::Issuer, command),
        IssuerCommand::CredentialTypes(command) => {
            let api = ctx.api()?;
            let tenant = ctx.tenant(&api)?;
            match command {
                CredentialTypesCommand::Get { id } => {
                    let types =
                        api.get(&["tenants", &tenant, "issuers", &id, "credential-types"])?;
                    show_types(ctx.out, &types);
                }
                CredentialTypesCommand::Set { id, types, forms } => {
                    let forms: Map<String, Value> = forms
                        .into_iter()
                        .map(|(kind, form)| (kind, json!(form)))
                        .collect();
                    let types = api.put(
                        &["tenants", &tenant, "issuers", &id, "credential-types"],
                        &json!({"types": types, "forms": forms}),
                    )?;
                    show_types(ctx.out, &types);
                }
            }
            Ok(())
        }
        IssuerCommand::StatusList(command) => {
            let api = ctx.api()?;
            let tenant = ctx.tenant(&api)?;
            match command {
                StatusListCommand::List { id } => {
                    let lists = api.get(&["tenants", &tenant, "issuers", &id, "status-lists"])?;
                    show_lists(ctx.out, &lists);
                }
                StatusListCommand::Sign { id, status_list } => {
                    let mut body = wallet::asking(ctx.locale());
                    if let Some(list) = status_list {
                        body["status_list_id"] = json!(list);
                    }
                    let request = api.post(
                        &["tenants", &tenant, "issuers", &id, "status-lists", "sign"],
                        &body,
                    )?;
                    wallet::wait(&api, &request, "sign the status list")?;
                    notice("Signed: verifiers read it at its address.");
                    let lists = api.get(&["tenants", &tenant, "issuers", &id, "status-lists"])?;
                    show_lists(ctx.out, &lists);
                }
            }
            Ok(())
        }
    }
}

fn show_lists(out: Output, lists: &Value) {
    if out.is_json() {
        return out.document(lists);
    }
    let items = lists["items"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if items.is_empty() {
        notice("No status list yet: its signer signs one before its first credential.");
    } else {
        out.list(
            &lists["items"],
            &[
                col("ID", "id"),
                col("Address", "uri"),
                col("Used", "used"),
                col("Size", "size"),
                col("Revoked", "revoked"),
                col("Suspended", "suspended"),
                col("Signed", "signed_at"),
                col("To sign", "needs_signing"),
            ],
        );
    }
    if lists["can_sign"] != json!(true) {
        notice(if lists["signer_needed"] == json!(true) {
            "The issuer has no signer: name one with `almena issuer signing set`."
        } else {
            "Only the issuer's signer signs its status lists."
        });
    }
}

fn show_types(out: Output, types: &Value) {
    if out.is_json() {
        return out.document(types);
    }
    let kinds = types["types"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if kinds.is_empty() {
        notice("It grants no credential type.");
        return;
    }
    let rows: Vec<Value> = kinds
        .iter()
        .map(|kind| {
            let form = kind.as_str().map_or(&Value::Null, |k| &types["forms"][k]);
            json!({"type": kind, "form": form})
        })
        .collect();
    out.list(
        &Value::Array(rows),
        &[col("Type", "type"), col("Form (offer)", "form")],
    );
}
