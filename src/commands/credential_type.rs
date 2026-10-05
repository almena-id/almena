//! `almena credential-type`: the tenant's credential types. The trust anchor's
//! are Almena's catalogue, for every tenant's issuers and forms; any other
//! tenant's are its own (`custom:{key}`), for its issuers and forms, when its
//! subscription allows it (the API refuses with `subscription_required`).

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Map, Value, json};

use crate::commands::{body_from, parse_text, texts};
use crate::context::Context;
use crate::output::{Column, col, notice};

#[derive(Debug, Subcommand)]
pub enum CredentialTypeCommand {
    /// List the tenant's own credential types (the anchor's: Almena's), newest first.
    List,
    /// Add a credential type (the anchor's: to Almena's catalogue), from flags, a
    /// JSON file, or both (flags win).
    Create(Box<CreateArgs>),
    /// Change one: only what is passed, never its key (a type in use only takes
    /// new words and optional claims).
    Update(Box<UpdateArgs>),
    /// Delete one (refused while an issuer grants it, a form asks for it or an
    /// application was made for it).
    Delete {
        /// Its id.
        id: String,
    },
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Its key (`[a-z][a-z0-9_]*`); it names its `vct`.
    #[arg(long)]
    key: Option<String>,
    #[command(flatten)]
    fields: TypeArgs,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Its id.
    id: String,
    #[command(flatten)]
    fields: TypeArgs,
}

/// What a type is made of, as flags over an optional JSON file.
#[derive(Debug, Args)]
pub struct TypeArgs {
    /// The type as JSON (`-` for stdin).
    #[arg(long, value_name = "FILE")]
    file: Option<PathBuf>,
    /// Its name in a language, LANG=TEXT; repeat per language.
    #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text)]
    label: Vec<(String, String)>,
    /// What it states, in a language, LANG=TEXT; repeat per language.
    #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text)]
    description: Vec<(String, String)>,
    /// One of the catalogue's credential categories.
    #[arg(long)]
    category: Option<String>,
    /// The standard or model it follows.
    #[arg(long)]
    source: Option<String>,
    /// A field of the catalogue it always carries; repeat for more.
    #[arg(long, value_name = "FIELD")]
    claim: Vec<String>,
    /// A field of the catalogue it may carry; repeat for more.
    #[arg(long, value_name = "FIELD")]
    optional_claim: Vec<String>,
    /// Who issues it.
    #[arg(long, value_enum)]
    issuance: Option<Issuance>,
    /// `external` types: how their framework names them.
    #[arg(long)]
    vct: Option<String>,
    /// Its W3C type, as in `VerifiableCredential` lists.
    #[arg(long)]
    w3c_type: Option<String>,
    /// Its mdoc doctype.
    #[arg(long)]
    mdoc_doctype: Option<String>,
}

impl TypeArgs {
    /// The body to send: the file's, the flags over it.
    fn body(self) -> anyhow::Result<Map<String, Value>> {
        let mut body = body_from(self.file.as_deref())?;
        if !self.label.is_empty() {
            body.insert("labels".into(), texts(&self.label));
        }
        if !self.description.is_empty() {
            body.insert("descriptions".into(), texts(&self.description));
        }
        if let Some(category) = self.category {
            body.insert("category".into(), json!(category));
        }
        if let Some(source) = self.source {
            body.insert("source".into(), json!(source));
        }
        if !self.claim.is_empty() || !self.optional_claim.is_empty() {
            let claims = self
                .claim
                .iter()
                .map(|f| json!({"field": f, "required": true}))
                .chain(
                    self.optional_claim
                        .iter()
                        .map(|f| json!({"field": f, "required": false})),
                )
                .collect();
            body.insert("claims".into(), Value::Array(claims));
        }
        if let Some(issuance) = self.issuance {
            body.insert("issuance".into(), json!(issuance.api()));
        }
        if let Some(vct) = self.vct {
            body.insert("vct".into(), json!(vct));
        }
        if let Some(w3c_type) = self.w3c_type {
            body.insert("w3c_type".into(), json!(w3c_type));
        }
        if let Some(doctype) = self.mdoc_doctype {
            body.insert("mdoc_doctype".into(), json!(doctype));
        }
        Ok(body)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Issuance {
    /// Any tenant's issuer grants it.
    Almena,
    /// Issued under a framework of its own; only asked for.
    External,
}

impl Issuance {
    fn api(self) -> &'static str {
        match self {
            Self::Almena => "almena",
            Self::External => "external",
        }
    }
}

const COLUMNS: &[Column] = &[
    col("ID", "id"),
    col("Key", "key"),
    col("Name", "type.labels"),
    col("Category", "type.category"),
    col("Issuance", "type.issuance"),
    col("Updated", "updated_at"),
];

pub fn run(ctx: &Context, command: CredentialTypeCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        CredentialTypeCommand::List => ctx.out.list(
            &api.get(&["tenants", &tenant, "credential-types"])?,
            COLUMNS,
        ),
        CredentialTypeCommand::Create(args) => {
            let CreateArgs { key, fields } = *args;
            let mut body = fields.body()?;
            if let Some(key) = key {
                body.insert("key".into(), json!(key));
            }
            let made = api.post(
                &["tenants", &tenant, "credential-types"],
                &Value::Object(body),
            )?;
            ctx.out.item(&made, COLUMNS);
            notice(format!(
                "Its vct: {}",
                ctx.out.cell(&made["type"]["formats"]["dc+sd-jwt"]["vct"])
            ));
        }
        CredentialTypeCommand::Update(args) => {
            let UpdateArgs { id, fields } = *args;
            let body = fields.body()?;
            if body.is_empty() {
                anyhow::bail!("nothing to change: pass a flag or --file");
            }
            let changed = api.patch(
                &["tenants", &tenant, "credential-types", &id],
                &Value::Object(body),
            )?;
            ctx.out.item(&changed, COLUMNS);
        }
        CredentialTypeCommand::Delete { id } => {
            ctx.confirm("Delete that credential type?")?;
            api.delete(&["tenants", &tenant, "credential-types", &id])?;
            notice("Credential type deleted.");
        }
    }
    Ok(())
}
