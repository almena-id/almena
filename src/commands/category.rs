//! `almena category`: the categories Almena's catalogue files its fields and
//! credential types under. Only the trust anchor keeps them (the API refuses
//! any other tenant with `anchor_only`); every tenant reads them in
//! `almena catalog fields` and `almena catalog credentials`.

use clap::{Subcommand, ValueEnum};
use serde_json::json;

use crate::commands::{parse_text, texts};
use crate::context::Context;
use crate::output::{Column, col, notice};

#[derive(Debug, Subcommand)]
pub enum CategoryCommand {
    /// List them, with how much is filed under each.
    List,
    /// Add one.
    Create {
        /// What it files.
        #[arg(long, value_enum)]
        kind: Kind,
        /// Its key (`[a-z][a-z0-9_]*`), once per kind.
        #[arg(long)]
        key: String,
        /// Its name in a language, LANG=TEXT; one per language of the portal.
        #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text, required = true)]
        label: Vec<(String, String)>,
    },
    /// Rename one; its kind and key never change.
    Update {
        /// Its id.
        id: String,
        /// Its name in a language, LANG=TEXT; one per language of the portal.
        #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text, required = true)]
        label: Vec<(String, String)>,
    },
    /// Delete one nothing is filed under.
    Delete {
        /// Its id.
        id: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Kind {
    Field,
    Credential,
}

impl Kind {
    fn api(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Credential => "credential",
        }
    }
}

const COLUMNS: &[Column] = &[
    col("ID", "id"),
    col("Kind", "kind"),
    col("Key", "key"),
    col("Name", "labels"),
    col("Uses", "uses"),
];

pub fn run(ctx: &Context, command: CategoryCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        CategoryCommand::List => ctx
            .out
            .list(&api.get(&["tenants", &tenant, "categories"])?, COLUMNS),
        CategoryCommand::Create { kind, key, label } => {
            let made = api.post(
                &["tenants", &tenant, "categories"],
                &json!({"kind": kind.api(), "key": key, "labels": texts(&label)}),
            )?;
            ctx.out.item(&made, COLUMNS);
        }
        CategoryCommand::Update { id, label } => {
            let changed = api.patch(
                &["tenants", &tenant, "categories", &id],
                &json!({"labels": texts(&label)}),
            )?;
            ctx.out.item(&changed, COLUMNS);
        }
        CategoryCommand::Delete { id } => {
            ctx.confirm("Delete that category?")?;
            api.delete(&["tenants", &tenant, "categories", &id])?;
            notice("Category deleted.");
        }
    }
    Ok(())
}
