//! `almena catalog`: Almena's public catalogues — the fields forms may ask
//! for, the credential types — and what tenants have published. No sign-in.

use clap::Subcommand;

use crate::commands::{PageArgs, paged};
use crate::context::Context;
use crate::output::{Column, col};

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// Almena's field catalogue: the only fields a form may ask for.
    Fields,
    /// Almena's credential types: what issuers grant and forms ask for.
    Credentials,
    /// Published issuers, newest first.
    Issuers(PageArgs),
    /// Published verifiers, newest first.
    Verifiers(PageArgs),
    /// Published mediators, newest first.
    Mediators(PageArgs),
    /// An issuer's offer: the credential type and the form to apply with.
    Offer {
        /// The issuer's slug (`iss_…`).
        issuer: String,
        /// The credential type.
        #[arg(value_name = "TYPE")]
        kind: String,
    },
}

const PUBLISHED: &[Column] = &[
    col("Name", "name"),
    col("DID", "did"),
    col("Tenant", "tenant.name"),
    col("Published", "published_at"),
];

pub fn run(ctx: &Context, command: CatalogCommand) -> anyhow::Result<()> {
    let api = ctx.public_api()?;
    match command {
        CatalogCommand::Fields => {
            let catalog = api.get(&["catalog", "fields"])?;
            if ctx.out.is_json() {
                ctx.out.document(&catalog);
                return Ok(());
            }
            ctx.out.list(
                &catalog["fields"],
                &[
                    col("ID", "id"),
                    col("Category", "category"),
                    col("Type", "type"),
                    col("Label", "labels"),
                    col("Source", "source"),
                ],
            );
        }
        CatalogCommand::Credentials => {
            let catalog = api.get(&["catalog", "credentials"])?;
            if ctx.out.is_json() {
                ctx.out.document(&catalog);
                return Ok(());
            }
            ctx.out.list(
                &catalog["types"],
                &[
                    col("ID", "id"),
                    col("Category", "category"),
                    col("Label", "labels"),
                    col("Issuance", "issuance"),
                ],
            );
        }
        CatalogCommand::Issuers(page) => {
            let items = paged(&api, &["catalog", "issuers"], &page)?;
            ctx.out.list(
                &items,
                &[
                    col("Slug", "slug"),
                    col("Name", "name"),
                    col("Grants", "credential_types"),
                    col("Offers", "offers"),
                    col("DID", "did"),
                    col("Tenant", "tenant.name"),
                ],
            );
        }
        CatalogCommand::Verifiers(page) => {
            ctx.out
                .list(&paged(&api, &["catalog", "verifiers"], &page)?, PUBLISHED);
        }
        CatalogCommand::Mediators(page) => {
            let items = paged(&api, &["catalog", "mediators"], &page)?;
            ctx.out.list(
                &items,
                &[
                    col("Name", "name"),
                    col("URL", "url"),
                    col("DID", "did"),
                    col("Tenant", "tenant.name"),
                ],
            );
        }
        CatalogCommand::Offer { issuer, kind } => {
            ctx.out
                .document(&api.get(&["catalog", "issuers", &issuer, "offers", &kind])?);
        }
    }
    Ok(())
}
