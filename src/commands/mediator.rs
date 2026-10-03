//! `almena mediator`: the tenant's mediators, and those it may pick.

use clap::{ArgAction, Subcommand};
use serde_json::{Map, Value, json};

use crate::commands::described::{self, Kind};
use crate::commands::{PageArgs, paged};
use crate::context::Context;
use crate::output::{Output, col, notice};

#[derive(Debug, Subcommand)]
pub enum MediatorCommand {
    /// List the tenant's mediators, newest first.
    List(PageArgs),
    /// Show one, with its DID.
    Get {
        /// Its id.
        id: String,
    },
    /// Register a mediator listening on a subdomain of one of the tenant's verified domains.
    Create {
        /// Its name (1–200 characters).
        #[arg(long)]
        name: String,
        /// Where it listens: one label or more (`relay`, `eu.relay`) under --domain.
        #[arg(long)]
        subdomain: String,
        /// The verified domain it listens under, by id (see `almena domain list`).
        #[arg(long)]
        domain: String,
        /// Offer it to every tenant once published.
        #[arg(long)]
        public: bool,
    },
    /// Change its name, address or whether it is offered to every tenant.
    Update {
        /// Its id.
        id: String,
        /// Its new name.
        #[arg(long)]
        name: Option<String>,
        /// Move it to this subdomain (`relay`, `eu.relay`) of --domain.
        #[arg(long, requires = "domain")]
        subdomain: Option<String>,
        /// The verified domain it moves under, by id (see `almena domain list`).
        #[arg(long, requires = "subdomain")]
        domain: Option<String>,
        /// Offer it to every tenant (true) or not (false).
        #[arg(long, action = ArgAction::Set, value_name = "BOOL")]
        public: Option<bool>,
    },
    /// Delete it and its identity (admins).
    Delete {
        /// Its id.
        id: String,
    },
    /// Publish it: endorse it from your wallet (whoever signs as the tenant).
    Publish {
        /// Its id.
        id: String,
    },
    /// Take it back to a draft (admins).
    Unpublish {
        /// Its id.
        id: String,
    },
    /// The mediators the tenant, its issuers and verifiers may pick.
    Choices,
}

pub fn run(ctx: &Context, command: MediatorCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        MediatorCommand::List(page) => {
            let items = paged(&api, &["tenants", &tenant, "mediators"], &page)?;
            ctx.out.list(
                &items,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("URL", "url"),
                    col("Public", "public"),
                    col("Signature", "signature"),
                    col("Published", "published_at"),
                ],
            );
        }
        MediatorCommand::Get { id } => {
            show(ctx.out, &api.get(&["tenants", &tenant, "mediators", &id])?)
        }
        MediatorCommand::Create {
            name,
            subdomain,
            domain,
            public,
        } => {
            let made = api.post(
                &["tenants", &tenant, "mediators"],
                &json!({"name": name, "subdomain": subdomain, "domain_id": domain, "public": public}),
            )?;
            show(ctx.out, &made);
        }
        MediatorCommand::Update {
            id,
            name,
            subdomain,
            domain,
            public,
        } => {
            let mut body = Map::new();
            if let Some(name) = name {
                body.insert("name".into(), json!(name));
            }
            if let (Some(subdomain), Some(domain)) = (subdomain, domain) {
                body.insert("subdomain".into(), json!(subdomain));
                body.insert("domain_id".into(), json!(domain));
            }
            if let Some(public) = public {
                body.insert("public".into(), json!(public));
            }
            if body.is_empty() {
                anyhow::bail!(
                    "nothing to change: pass --name, --subdomain with --domain, or --public"
                );
            }
            show(
                ctx.out,
                &api.patch(
                    &["tenants", &tenant, "mediators", &id],
                    &Value::Object(body),
                )?,
            );
        }
        MediatorCommand::Delete { id } => {
            described::delete(ctx, &api, &tenant, Kind::Mediator, &id)?
        }
        MediatorCommand::Publish { id } => {
            described::publish(ctx, &api, &tenant, Kind::Mediator, &id, show)?;
        }
        MediatorCommand::Unpublish { id } => {
            described::unpublish(ctx, &api, &tenant, Kind::Mediator, &id, show)?;
        }
        MediatorCommand::Choices => {
            let choices = api.get(&["tenants", &tenant, "mediator-choices"])?;
            ctx.out.list(
                &choices,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("URL", "url"),
                    col("Own", "own"),
                    col("Published", "published"),
                ],
            );
            if !ctx.out.is_json() {
                notice("Pick one with --mediator on `tenant update`, `issuer create`…");
            }
        }
    }
    Ok(())
}

fn show(out: Output, mediator: &Value) {
    out.item(
        mediator,
        &[
            col("ID", "id"),
            col("Name", "name"),
            col("URL", "url"),
            col("Public", "public"),
            col("DID", "did"),
            col("Signature", "signature"),
            col("Published", "published_at"),
            col("Endorsed until", "endorsed_until"),
            col("Created", "created_at"),
        ],
    );
}
