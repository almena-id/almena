//! `almena value-domain`: the value lists coded fields draw on (countries,
//! languages, file formats…). Only the trust anchor keeps them (the API
//! refuses any other tenant with `anchor_only`); every tenant reads them in
//! `almena catalog fields`. Codes travel in a JSON file: long lists.

use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::commands::{body_from, parse_text, texts};
use crate::context::Context;
use crate::output::{Column, col, notice};

#[derive(Debug, Subcommand)]
pub enum ValueDomainCommand {
    /// List them, with how many codes each has and how many fields draw on it.
    List,
    /// Add one, from a JSON file (`{key, labels, source, codes}`), flags over it.
    Create {
        /// The value list as JSON (`-` for stdin); `codes` is `[{value, labels, media_type?}]`.
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// Its key (`[a-z][a-z0-9_]*`).
        #[arg(long)]
        key: Option<String>,
        /// Its name in a language, LANG=TEXT; one per language of the portal.
        #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text)]
        label: Vec<(String, String)>,
        /// The standard or list it comes from.
        #[arg(long)]
        source: Option<String>,
    },
    /// Change one: only what is passed; `codes` in the file replaces the whole
    /// list (while a field draws on it, every code it had must stay).
    Update {
        /// Its id.
        id: String,
        /// What changes, as JSON (`-` for stdin).
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// Its name in a language, LANG=TEXT; one per language of the portal.
        #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text)]
        label: Vec<(String, String)>,
        /// The standard or list it comes from.
        #[arg(long)]
        source: Option<String>,
    },
    /// Delete one no field draws on.
    Delete {
        /// Its id.
        id: String,
    },
}

const COLUMNS: &[Column] = &[
    col("ID", "id"),
    col("Key", "key"),
    col("Name", "labels"),
    col("Source", "source"),
    col("Fields", "uses"),
];

pub fn run(ctx: &Context, command: ValueDomainCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        ValueDomainCommand::List => ctx
            .out
            .list(&api.get(&["tenants", &tenant, "value-domains"])?, COLUMNS),
        ValueDomainCommand::Create {
            file,
            key,
            label,
            source,
        } => {
            let mut body = body_from(file.as_deref())?;
            if let Some(key) = key {
                body.insert("key".into(), json!(key));
            }
            if !label.is_empty() {
                body.insert("labels".into(), texts(&label));
            }
            if let Some(source) = source {
                body.insert("source".into(), json!(source));
            }
            let made = api.post(&["tenants", &tenant, "value-domains"], &Value::Object(body))?;
            ctx.out.item(&made, COLUMNS);
        }
        ValueDomainCommand::Update {
            id,
            file,
            label,
            source,
        } => {
            let mut body = body_from(file.as_deref())?;
            if !label.is_empty() {
                body.insert("labels".into(), texts(&label));
            }
            if let Some(source) = source {
                body.insert("source".into(), json!(source));
            }
            if body.is_empty() {
                anyhow::bail!("nothing to change: pass a flag or --file");
            }
            let changed = api.patch(
                &["tenants", &tenant, "value-domains", &id],
                &Value::Object(body),
            )?;
            ctx.out.item(&changed, COLUMNS);
        }
        ValueDomainCommand::Delete { id } => {
            ctx.confirm("Delete that value list?")?;
            api.delete(&["tenants", &tenant, "value-domains", &id])?;
            notice("Value list deleted.");
        }
    }
    Ok(())
}
