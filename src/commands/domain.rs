//! `almena domain`: the tenant's linked domains, proved by a DNS TXT record.

use clap::Subcommand;
use serde_json::{Value, json};

use crate::context::Context;
use crate::output::{Output, col, notice};

#[derive(Debug, Subcommand)]
pub enum DomainCommand {
    /// List the tenant's linked domains.
    List,
    /// Link a domain (admins): it comes with the TXT record that proves it.
    Add {
        /// The domain (`acme.com`; a URL is taken down to its host).
        domain: String,
    },
    /// Look for its TXT record (admins): found, the domain is verified; gone, a
    /// verified one stops being verified and leaves the DID document.
    Check {
        /// The domain's id.
        id: String,
    },
    /// Unlink a domain (admins).
    Remove {
        /// The domain's id.
        id: String,
    },
}

pub fn run(ctx: &Context, command: DomainCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        DomainCommand::List => {
            let domains = api.get(&["tenants", &tenant, "domains"])?;
            ctx.out.list(
                &domains,
                &[
                    col("ID", "id"),
                    col("Domain", "domain"),
                    col("Verified", "verified"),
                    col("Record", "dns_record.name"),
                    col("Value", "dns_record.value"),
                ],
            );
        }
        DomainCommand::Add { domain } => {
            let linked = api.post(&["tenants", &tenant, "domains"], &json!({"domain": domain}))?;
            show(ctx.out, &linked);
            if !ctx.out.is_json() {
                notice(format!(
                    "Add this TXT record to its DNS, then run `almena domain check {}`:\n  {} TXT \"{}\"",
                    ctx.out.cell(&linked["id"]),
                    ctx.out.cell(&linked["dns_record"]["name"]),
                    ctx.out.cell(&linked["dns_record"]["value"]),
                ));
            }
        }
        DomainCommand::Check { id } => {
            let checked = api.post(&["tenants", &tenant, "domains", &id, "check"], &Value::Null)?;
            show(ctx.out, &checked);
            if checked["verified"] == true {
                notice(
                    "Verified. The tenant's DID document names it now: sign it (`almena signature list`).",
                );
            }
        }
        DomainCommand::Remove { id } => {
            ctx.confirm("Unlink that domain?")?;
            api.delete(&["tenants", &tenant, "domains", &id])?;
            notice("Domain unlinked.");
        }
    }
    Ok(())
}

fn show(out: Output, domain: &Value) {
    out.item(
        domain,
        &[
            col("ID", "id"),
            col("Domain", "domain"),
            col("Verified", "verified"),
            col("Verified at", "verified_at"),
            col("Record", "dns_record.name"),
            col("Type", "dns_record.type"),
            col("Value", "dns_record.value"),
            col("Linked", "created_at"),
        ],
    );
}
