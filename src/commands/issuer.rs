//! `almena issuer`: the tenant's issuers, and the credential types they grant.

use clap::Subcommand;
use serde_json::{Map, Value, json};

use crate::commands::described::{self, DescribedCommand, Kind};
use crate::context::Context;
use crate::output::{Output, notice};

#[derive(Debug, Subcommand)]
pub enum IssuerCommand {
    #[command(flatten)]
    Described(DescribedCommand),
    /// The credential types it grants, and the form each is applied for with.
    #[command(subcommand)]
    CredentialTypes(CredentialTypesCommand),
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
        &[
            crate::output::col("Type", "type"),
            crate::output::col("Form (offer)", "form"),
        ],
    );
}
