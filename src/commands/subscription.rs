//! `almena subscription`: the tenant's subscription and the features it gives;
//! for the trust anchor's admins, every other tenant's (an account's), set
//! and removed by hand until payments drive them.

use clap::{Subcommand, ValueEnum};
use serde_json::{Map, Value, json};

use crate::commands::{PageArgs, paged_with};
use crate::context::Context;
use crate::output::{Column, col, notice};

#[derive(Debug, Subcommand)]
pub enum SubscriptionCommand {
    /// The tenant's subscription (plan, status, until when, features); with an
    /// account id, that account's (the trust anchor's admins).
    Get {
        /// An account's id.
        account: Option<String>,
    },
    /// The trust anchor's admins: every other account with its subscription.
    List {
        #[command(flatten)]
        page: PageArgs,
        /// Only those whose name (any case) has this in it, or with this slug.
        #[arg(long, value_name = "TEXT")]
        search: Option<String>,
        /// Only those with a subscription (in any state), or without.
        #[arg(long, value_enum)]
        subscribed: Option<Subscribed>,
    },
    /// The trust anchor's admins: set an account's subscription.
    Set {
        /// The account's id.
        account: String,
        /// Where it stands.
        #[arg(long, value_enum)]
        status: Status,
        /// Its plan.
        #[arg(long, default_value = "standard")]
        plan: String,
        /// Paid through this moment (RFC 3339, e.g. 2026-12-31T23:59:59Z); none: open-ended.
        #[arg(long, value_name = "WHEN")]
        until: Option<String>,
        /// A note only the anchor reads (an invoice, an agreement).
        #[arg(long)]
        note: Option<String>,
    },
    /// The trust anchor's admins: remove an account's subscription (back to the free use).
    Remove {
        /// The account's id.
        account: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Subscribed {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Status {
    Active,
    PastDue,
    Canceled,
}

impl Status {
    fn api(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::PastDue => "past_due",
            Self::Canceled => "canceled",
        }
    }
}

const COLUMNS: &[Column] = &[
    col("Plan", "plan"),
    col("Status", "status"),
    col("In force", "in_force"),
    col("Until", "current_period_end"),
    col("Features", "features"),
];

const ACCOUNTS: &[Column] = &[
    col("ID", "id"),
    col("Name", "name"),
    col("Members", "members"),
    col("Status", "subscription.status"),
    col("In force", "subscription.in_force"),
    col("Until", "subscription.current_period_end"),
];

pub fn run(ctx: &Context, command: SubscriptionCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        SubscriptionCommand::Get { account: None } => ctx
            .out
            .item(&api.get(&["tenants", &tenant, "subscription"])?, COLUMNS),
        SubscriptionCommand::Get {
            account: Some(account),
        } => ctx.out.item(
            &api.get(&["tenants", &tenant, "accounts", &account])?,
            ACCOUNTS,
        ),
        SubscriptionCommand::List {
            page,
            search,
            subscribed,
        } => {
            let mut filters = Vec::new();
            if let Some(text) = search {
                filters.push(("q", text));
            }
            if let Some(subscribed) = subscribed {
                let value = match subscribed {
                    Subscribed::Yes => "yes",
                    Subscribed::No => "no",
                };
                filters.push(("subscribed", value.to_owned()));
            }
            let items = paged_with(&api, &["tenants", &tenant, "accounts"], &page, &filters)?;
            ctx.out.list(&items, ACCOUNTS);
        }
        SubscriptionCommand::Set {
            account,
            status,
            plan,
            until,
            note,
        } => {
            let mut body = Map::new();
            body.insert("plan".into(), json!(plan));
            body.insert("status".into(), json!(status.api()));
            body.insert("current_period_end".into(), json!(until));
            if let Some(note) = note {
                body.insert("note".into(), json!(note));
            }
            let set = api.put(
                &["tenants", &tenant, "accounts", &account, "subscription"],
                &Value::Object(body),
            )?;
            ctx.out.item(&set, ACCOUNTS);
        }
        SubscriptionCommand::Remove { account } => {
            ctx.confirm("Remove that account's subscription?")?;
            api.delete(&["tenants", &tenant, "accounts", &account, "subscription"])?;
            notice("Subscription removed: the account is back to the free use.");
        }
    }
    Ok(())
}
