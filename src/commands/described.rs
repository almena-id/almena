//! What issuers and verifiers share (and, publishing and deleting, mediators):
//! the directory's items, registered in a tenant, published by endorsing them
//! from the wallet of whoever signs as the tenant.

use clap::{Args, Subcommand};
use serde_json::{Map, Value, json};

use crate::client::Client;
use crate::commands::{PageArgs, paged};
use crate::context::Context;
use crate::output::{Column, Output, col, notice};
use crate::wallet;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Issuer,
    Verifier,
    Mediator,
}

impl Kind {
    /// Its collection in the API's paths.
    pub fn segment(self) -> &'static str {
        match self {
            Self::Issuer => "issuers",
            Self::Verifier => "verifiers",
            Self::Mediator => "mediators",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Issuer => "issuer",
            Self::Verifier => "verifier",
            Self::Mediator => "mediator",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum DescribedCommand {
    /// List them, newest first.
    List(PageArgs),
    /// Show one, with its DID and DID document.
    Get {
        /// Its id.
        id: String,
    },
    /// Register one (a draft until it is published).
    Create {
        /// Its name (1–200 characters).
        #[arg(long)]
        name: String,
        /// What it is for.
        #[arg(long)]
        description: Option<String>,
        /// The mediator it receives messages through (see `almena mediator choices`).
        #[arg(long)]
        mediator: Option<String>,
    },
    /// Change its name, description or mediator; its DID stays.
    Update(UpdateArgs),
    /// Delete it and its identity: its DID stops resolving for good (admins).
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
    /// How it signs: the member whose wallet signs for it.
    #[command(subcommand)]
    Signing(SigningCommand),
    /// Its queue at the broker: where its back office reads what happens to it.
    #[command(subcommand)]
    Queue(QueueCommand),
}

#[derive(Debug, Subcommand)]
pub enum QueueCommand {
    /// Show whether it has one, and how its back office connects.
    Get {
        /// Its id.
        id: String,
    },
    /// Make it and the user that reads it (admins); prints the password, once.
    Create {
        /// Its id.
        id: String,
    },
    /// Give its user a new password (admins); the old one stops working.
    Rotate {
        /// Its id.
        id: String,
    },
    /// Delete it, with every message still in it, and its user (admins).
    Delete {
        /// Its id.
        id: String,
    },
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Its id.
    pub id: String,
    /// Its new name.
    #[arg(long)]
    pub name: Option<String>,
    /// Its new description.
    #[arg(long, conflicts_with = "no_description")]
    pub description: Option<String>,
    /// Remove its description.
    #[arg(long)]
    pub no_description: bool,
    /// Its new mediator.
    #[arg(long, conflicts_with = "no_mediator")]
    pub mediator: Option<String>,
    /// Remove its mediator.
    #[arg(long)]
    pub no_mediator: bool,
}

#[derive(Debug, Subcommand)]
pub enum SigningCommand {
    /// Show how it signs.
    Get {
        /// Its id.
        id: String,
    },
    /// Set who signs for it (admins).
    Set {
        /// Its id.
        id: String,
        /// The member who signs alone (`single_user`), by user id.
        #[arg(long, conflicts_with = "none", required_unless_present = "none")]
        user: Option<String>,
        /// Leave it without a signing system.
        #[arg(long)]
        none: bool,
    },
}

pub fn run(ctx: &Context, kind: Kind, command: DescribedCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    let collection = kind.segment();
    match command {
        DescribedCommand::List(page) => {
            let items = paged(&api, &["tenants", &tenant, collection], &page)?;
            ctx.out.list(&items, LIST);
        }
        DescribedCommand::Get { id } => {
            show(ctx.out, &api.get(&["tenants", &tenant, collection, &id])?);
        }
        DescribedCommand::Create {
            name,
            description,
            mediator,
        } => {
            let made = api.post(
                &["tenants", &tenant, collection],
                &json!({"name": name, "description": description, "mediator_id": mediator}),
            )?;
            show(ctx.out, &made);
            notice(format!(
                "Registered. Publish it with `almena {} publish {}` once it is signed.",
                kind.name(),
                made["id"].as_str().unwrap_or_default()
            ));
        }
        DescribedCommand::Update(args) => {
            let mut body = Map::new();
            if let Some(name) = args.name {
                body.insert("name".into(), json!(name));
            }
            if let Some(description) = args.description {
                body.insert("description".into(), json!(description));
            } else if args.no_description {
                body.insert("description".into(), Value::Null);
            }
            if let Some(mediator) = args.mediator {
                body.insert("mediator_id".into(), json!(mediator));
            } else if args.no_mediator {
                body.insert("mediator_id".into(), Value::Null);
            }
            if body.is_empty() {
                anyhow::bail!("nothing to change: pass --name, --description or --mediator");
            }
            let updated = api.patch(
                &["tenants", &tenant, collection, &args.id],
                &Value::Object(body),
            )?;
            show(ctx.out, &updated);
        }
        DescribedCommand::Delete { id } => delete(ctx, &api, &tenant, kind, &id)?,
        DescribedCommand::Publish { id } => publish(ctx, &api, &tenant, kind, &id, show)?,
        DescribedCommand::Unpublish { id } => unpublish(ctx, &api, &tenant, kind, &id, show)?,
        DescribedCommand::Signing(SigningCommand::Get { id }) => {
            let signing = api.get(&["tenants", &tenant, collection, &id, "signing"])?;
            show_signing(ctx.out, &signing);
        }
        DescribedCommand::Queue(command) => queue(ctx, &api, &tenant, collection, command)?,
        DescribedCommand::Signing(SigningCommand::Set { id, user, .. }) => {
            let body = match user {
                Some(user) => json!({"system": "single_user", "user_id": user}),
                None => json!({"system": null}),
            };
            let signing = api.put(&["tenants", &tenant, collection, &id, "signing"], &body)?;
            show_signing(ctx.out, &signing);
        }
    }
    Ok(())
}

const LIST: &[Column] = &[
    col("ID", "id"),
    col("Name", "name"),
    col("Mediator", "mediator.name"),
    col("Signature", "signature"),
    col("Published", "published_at"),
    col("Created", "created_at"),
];

pub fn show(out: Output, item: &Value) {
    out.item(
        item,
        &[
            col("ID", "id"),
            col("Name", "name"),
            col("Description", "description"),
            col("DID", "did"),
            col("Mediator", "mediator.name"),
            col("Signature", "signature"),
            col("Published", "published_at"),
            col("Endorsed until", "endorsed_until"),
            col("Document", "document_url"),
            col("Created", "created_at"),
        ],
    );
}

fn queue(
    ctx: &Context,
    api: &Client,
    tenant: &str,
    collection: &str,
    command: QueueCommand,
) -> anyhow::Result<()> {
    match command {
        QueueCommand::Get { id } => {
            show_queue(
                ctx.out,
                &api.get(&["tenants", tenant, collection, &id, "queue"])?,
            );
        }
        QueueCommand::Create { id } => {
            let made = api.post(&["tenants", tenant, collection, &id, "queue"], &json!({}))?;
            show_queue(ctx.out, &made);
            notice("The password is shown this once: the registry does not keep it.");
        }
        QueueCommand::Rotate { id } => {
            let rotated = api.post(
                &["tenants", tenant, collection, &id, "queue", "access"],
                &json!({}),
            )?;
            show_queue(ctx.out, &rotated);
            notice("The old password no longer works; this one is shown this once.");
        }
        QueueCommand::Delete { id } => {
            ctx.confirm("Delete the queue, with every message still in it, and its user?")?;
            api.delete(&["tenants", tenant, collection, &id, "queue"])?;
            notice("Deleted.");
        }
    }
    Ok(())
}

fn show_queue(out: Output, queue: &Value) {
    out.item(
        queue,
        &[
            col("Queue", "queue"),
            col("User", "user"),
            col("Password", "password"),
            col("Address", "amqp_url"),
            col("Virtual host", "vhost"),
            col("Created", "created_at"),
        ],
    );
}

fn show_signing(out: Output, signing: &Value) {
    out.item(
        signing,
        &[
            col("System", "system"),
            col("Signer", "signer.id"),
            col("Email", "signer.email"),
            col("Alias", "signer.alias"),
            col("Member", "signer.member"),
            col("Has a wallet", "signer.wallet"),
        ],
    );
}

pub fn publish(
    ctx: &Context,
    api: &Client,
    tenant: &str,
    kind: Kind,
    id: &str,
    show: fn(Output, &Value),
) -> anyhow::Result<()> {
    let collection = kind.segment();
    let request = api.post(
        &["tenants", tenant, collection, id, "publish"],
        &wallet::asking(ctx.locale()),
    )?;
    wallet::wait(api, &request, &format!("endorse this {}", kind.name()))?;
    notice("Published.");
    show(ctx.out, &api.get(&["tenants", tenant, collection, id])?);
    Ok(())
}

pub fn unpublish(
    ctx: &Context,
    api: &Client,
    tenant: &str,
    kind: Kind,
    id: &str,
    show: fn(Output, &Value),
) -> anyhow::Result<()> {
    let item = api.post(
        &["tenants", tenant, kind.segment(), id, "unpublish"],
        &Value::Null,
    )?;
    notice("Back to a draft.");
    show(ctx.out, &item);
    Ok(())
}

pub fn delete(
    ctx: &Context,
    api: &Client,
    tenant: &str,
    kind: Kind,
    id: &str,
) -> anyhow::Result<()> {
    ctx.confirm(&format!(
        "Delete this {} and its identity? Its DID stops resolving for good.",
        kind.name()
    ))?;
    api.delete(&["tenants", tenant, kind.segment(), id])?;
    notice(format!("Deleted the {}.", kind.name()));
    Ok(())
}
