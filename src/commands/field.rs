//! `almena field`: the tenant's own fields, for what Almena's catalogue lacks
//! (`custom:{key}` in forms, never published).

use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::commands::{body_from, parse_text, texts};
use crate::context::Context;
use crate::output::{Column, col, notice};

#[derive(Debug, Subcommand)]
pub enum FieldCommand {
    /// List the tenant's own fields, newest first.
    List,
    /// Add a field of the tenant's own, from flags, a JSON file, or both (flags win).
    Create {
        /// The field as JSON (`-` for stdin), e.g. for `options`.
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// Its key (`[a-z][a-z0-9_]*`, never one of Almena's ids).
        #[arg(long)]
        key: Option<String>,
        /// Its type.
        #[arg(long = "type", value_enum)]
        kind: Option<FieldType>,
        /// Its label in a language, LANG=TEXT; repeat per language.
        #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text)]
        label: Vec<(String, String)>,
        /// The longest a text may be.
        #[arg(long)]
        max_length: Option<u32>,
        /// A regular expression a text must match.
        #[arg(long)]
        pattern: Option<String>,
        /// A file format it accepts (`file` fields); repeat for more.
        #[arg(long = "format", value_name = "FORMAT")]
        formats: Vec<String>,
    },
    /// Delete one (refused while a form asks for it).
    Delete {
        /// Its id.
        id: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FieldType {
    Text,
    Email,
    Phone,
    Date,
    Code,
    Codes,
    File,
}

impl FieldType {
    fn api(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Email => "email",
            Self::Phone => "phone",
            Self::Date => "date",
            Self::Code => "code",
            Self::Codes => "codes",
            Self::File => "file",
        }
    }
}

const COLUMNS: &[Column] = &[
    col("ID", "id"),
    col("Key", "key"),
    col("Ref", "ref"),
    col("Type", "field.type"),
    col("Label", "field.labels"),
    col("Updated", "updated_at"),
];

pub fn run(ctx: &Context, command: FieldCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        FieldCommand::List => ctx
            .out
            .list(&api.get(&["tenants", &tenant, "fields"])?, COLUMNS),
        FieldCommand::Create {
            file,
            key,
            kind,
            label,
            max_length,
            pattern,
            formats,
        } => {
            let mut body = body_from(file.as_deref())?;
            if let Some(key) = key {
                body.insert("key".into(), json!(key));
            }
            if let Some(kind) = kind {
                body.insert("type".into(), json!(kind.api()));
            }
            if !label.is_empty() {
                body.insert("labels".into(), texts(&label));
            }
            if let Some(max_length) = max_length {
                body.insert("max_length".into(), json!(max_length));
            }
            if let Some(pattern) = pattern {
                body.insert("pattern".into(), json!(pattern));
            }
            if !formats.is_empty() {
                body.insert("formats".into(), json!(formats));
            }
            let made = api.post(&["tenants", &tenant, "fields"], &Value::Object(body))?;
            ctx.out.item(&made, COLUMNS);
            notice(format!(
                "Forms ask for it as `{}`.",
                ctx.out.cell(&made["ref"])
            ));
        }
        FieldCommand::Delete { id } => {
            ctx.confirm("Delete that field?")?;
            api.delete(&["tenants", &tenant, "fields", &id])?;
            notice("Field deleted.");
        }
    }
    Ok(())
}
