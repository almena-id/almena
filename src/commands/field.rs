//! `almena field`: the tenant's own fields, for what Almena's catalogue lacks
//! (`custom:{key}` in forms, never published). The trust anchor's are Almena's
//! catalogue itself: forms ask for them by their key, and they carry a
//! category and the standard they are named after.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Map, Value, json};

use crate::commands::{body_from, parse_text, texts};
use crate::context::Context;
use crate::output::{Column, col, notice};

#[derive(Debug, Subcommand)]
pub enum FieldCommand {
    /// List the tenant's own fields, newest first.
    List,
    /// Add a field of the tenant's own, from flags, a JSON file, or both (flags win).
    Create {
        /// Its key (`[a-z][a-z0-9_]*`, never one of the anchor's).
        #[arg(long)]
        key: Option<String>,
        #[command(flatten)]
        fields: FieldArgs,
    },
    /// Change one: only what is passed, never its key. Its type, length,
    /// pattern, options and formats go together; while a form asks for it, it
    /// only takes more.
    Update {
        /// Its id.
        id: String,
        #[command(flatten)]
        fields: FieldArgs,
    },
    /// Delete one (refused while a form asks for it).
    Delete {
        /// Its id.
        id: String,
    },
}

/// What a field is made of, as flags over an optional JSON file.
#[derive(Debug, Args)]
pub struct FieldArgs {
    /// The field as JSON (`-` for stdin), e.g. for `options` or a group's `parts`.
    #[arg(long, value_name = "FILE")]
    file: Option<PathBuf>,
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
    /// `code` and `codes`: one of the anchor's value lists, in place of options.
    #[arg(long, value_name = "KEY")]
    domain: Option<String>,
    /// The trust anchor's: one of the catalogue's field categories.
    #[arg(long)]
    category: Option<String>,
    /// The trust anchor's: the standard it is named after.
    #[arg(long)]
    source: Option<String>,
}

impl FieldArgs {
    /// The body to send: the file's, the flags over it.
    fn body(self) -> anyhow::Result<Map<String, Value>> {
        let mut body = body_from(self.file.as_deref())?;
        if let Some(kind) = self.kind {
            body.insert("type".into(), json!(kind.api()));
        }
        if !self.label.is_empty() {
            body.insert("labels".into(), texts(&self.label));
        }
        if let Some(max_length) = self.max_length {
            body.insert("max_length".into(), json!(max_length));
        }
        if let Some(pattern) = self.pattern {
            body.insert("pattern".into(), json!(pattern));
        }
        if !self.formats.is_empty() {
            body.insert("formats".into(), json!(self.formats));
        }
        if let Some(domain) = self.domain {
            body.insert("domain".into(), json!(domain));
        }
        if let Some(category) = self.category {
            body.insert("category".into(), json!(category));
        }
        if let Some(source) = self.source {
            body.insert("source".into(), json!(source));
        }
        Ok(body)
    }
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
    /// The trust anchor's only; its `parts` go in the JSON file.
    Group,
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
            Self::Group => "group",
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
        FieldCommand::Create { key, fields } => {
            let mut body = fields.body()?;
            if let Some(key) = key {
                body.insert("key".into(), json!(key));
            }
            let made = api.post(&["tenants", &tenant, "fields"], &Value::Object(body))?;
            ctx.out.item(&made, COLUMNS);
            notice(format!(
                "Forms ask for it as `{}`.",
                ctx.out.cell(&made["ref"])
            ));
        }
        FieldCommand::Update { id, fields } => {
            let body = fields.body()?;
            if body.is_empty() {
                anyhow::bail!("nothing to change: pass a flag or --file");
            }
            let changed = api.patch(&["tenants", &tenant, "fields", &id], &Value::Object(body))?;
            ctx.out.item(&changed, COLUMNS);
        }
        FieldCommand::Delete { id } => {
            ctx.confirm("Delete that field?")?;
            api.delete(&["tenants", &tenant, "fields", &id])?;
            notice("Field deleted.");
        }
    }
    Ok(())
}
