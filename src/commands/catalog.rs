//! `almena catalog`: Almena's public catalogues — the fields forms may ask
//! for, the credential types — and what tenants have published. No sign-in.

use clap::Subcommand;

use crate::commands::{PageArgs, paged, paged_with};
use crate::context::Context;
use crate::output::{Column, col};

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// Almena's field catalogue: the only fields a form may ask for.
    Fields,
    /// Almena's credential types: what issuers grant and forms ask for.
    Credentials,
    /// Published issuers, newest first.
    Issuers {
        #[command(flatten)]
        page: PageArgs,
        /// Only those whose name (any case) or DID has this in it.
        #[arg(long, value_name = "TEXT")]
        search: Option<String>,
        /// Only those that grant this credential type.
        #[arg(long, value_name = "TYPE")]
        grants: Option<String>,
    },
    /// Published verifiers, newest first.
    Verifiers(PageArgs),
    /// Published mediators, newest first.
    Mediators(PageArgs),
    /// Every published issuer's offers: a credential type it grants on request.
    Offers(PageArgs),
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
        CatalogCommand::Issuers {
            page,
            search,
            grants,
        } => {
            let filters: Vec<(&str, String)> = [("q", search), ("grants", grants)]
                .into_iter()
                .filter_map(|(name, value)| value.map(|value| (name, value)))
                .collect();
            let items = paged_with(&api, &["catalog", "issuers"], &page, &filters)?;
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
        CatalogCommand::Offers(page) => {
            let items = paged(&api, &["catalog", "offers"], &page)?;
            ctx.out.list(
                &items,
                &[
                    col("Issuer", "issuer.slug"),
                    col("Name", "issuer.name"),
                    col("Type", "credential_type.id"),
                    col("Label", "credential_type.labels"),
                    col("Category", "credential_type.category.id"),
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
