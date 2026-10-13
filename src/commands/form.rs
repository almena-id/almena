//! `almena form`: the tenant's forms — what is asked of a person, whoever asks
//! it — changing and deleting them, and verifying credentials presented for one.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::{Map, Value, json};

use crate::commands::{body_from, parse_text, read_json, texts};
use crate::context::Context;
use crate::output::{Output, col, notice};

#[derive(Debug, Subcommand)]
pub enum FormCommand {
    /// List the tenant's forms, newest first.
    List,
    /// Show a form.
    Get {
        /// Its id.
        id: String,
    },
    /// Create a form from flags, a JSON file (the API's body), or both (flags win).
    Create(#[command(flatten)] FormArgs),
    /// Change a form: what is given replaces what it had (flags win over the
    /// file); in use, it may only take more than it took.
    Update {
        /// Its id.
        id: String,
        #[command(flatten)]
        form: FormArgs,
    },
    /// Delete a form no offer puts and nothing was made with.
    Delete {
        /// Its id.
        id: String,
    },
    /// The JSON Schema its answers meet.
    Schema {
        /// Its id.
        id: String,
    },
    /// The OpenID4VP DCQL query of the credentials it asks for.
    Dcql {
        /// Its id.
        id: String,
    },
    /// Verify credentials presented for it.
    Verify {
        /// Its id.
        id: String,
        /// `{vp_token, nonce, audience}` as JSON (`-` for stdin).
        #[arg(long, value_name = "FILE")]
        file: PathBuf,
    },
}

/// What a form is made of, as flags over an optional JSON file.
#[derive(Debug, Args)]
pub struct FormArgs {
    /// The form as JSON (`-` for stdin): `{name, description, fields, credentials}`.
    #[arg(long, value_name = "FILE")]
    file: Option<PathBuf>,
    /// Its name in a language, LANG=TEXT; repeat per language.
    #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text)]
    name: Vec<(String, String)>,
    /// Its description in a language, LANG=TEXT; repeat per language.
    #[arg(long, value_name = "LANG=TEXT", value_parser = parse_text)]
    description: Vec<(String, String)>,
    /// A required field it asks for: a catalogue id, or `custom:{key}`; repeat for more.
    #[arg(long, value_name = "REF")]
    field: Vec<String>,
    /// An optional field it asks for; repeat for more.
    #[arg(long, value_name = "REF")]
    optional_field: Vec<String>,
    /// A credential type it asks to be presented; repeat for more.
    #[arg(long, value_name = "TYPE")]
    credential: Vec<String>,
}

impl FormArgs {
    /// `body` with the file's keys, then the flags', put over it.
    fn over(&self, mut body: Map<String, Value>) -> anyhow::Result<Map<String, Value>> {
        body.extend(body_from(self.file.as_deref())?);
        if !self.name.is_empty() {
            body.insert("name".into(), texts(&self.name));
        }
        if !self.description.is_empty() {
            body.insert("description".into(), texts(&self.description));
        }
        if !self.field.is_empty() || !self.optional_field.is_empty() {
            let fields = self
                .field
                .iter()
                .map(|r| json!({"ref": r, "required": true}))
                .chain(
                    self.optional_field
                        .iter()
                        .map(|r| json!({"ref": r, "required": false})),
                )
                .collect();
            body.insert("fields".into(), Value::Array(fields));
        }
        if !self.credential.is_empty() {
            let wanted = self
                .credential
                .iter()
                .map(|kind| json!({"type": kind}))
                .collect();
            body.insert("credentials".into(), Value::Array(wanted));
        }
        Ok(body)
    }
}

/// A form as the API answers it, as the body that would make it again.
fn as_sent(form: &Value) -> Map<String, Value> {
    let mut body = Map::new();
    for key in ["name", "description", "fields"] {
        body.insert(key.into(), form[key].clone());
    }
    let credentials = form["credentials"]
        .as_array()
        .map(|all| {
            all.iter()
                .map(|entry| {
                    let mut entry = entry.clone();
                    if let Some(fields) = entry.as_object_mut() {
                        fields.remove("fills");
                    }
                    entry
                })
                .collect()
        })
        .unwrap_or_default();
    body.insert("credentials".into(), Value::Array(credentials));
    body
}

pub fn run(ctx: &Context, command: FormCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        FormCommand::List => {
            let forms = api.get(&["tenants", &tenant, "forms"])?;
            ctx.out.list(
                &forms,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("Fields", "fields"),
                    col("Credentials", "credentials"),
                    col("In use", "in_use"),
                    col("Updated", "updated_at"),
                ],
            );
        }
        FormCommand::Get { id } => show(ctx.out, &api.get(&["tenants", &tenant, "forms", &id])?),
        FormCommand::Create(form) => {
            let body = form.over(Map::new())?;
            if !body.contains_key("name") {
                anyhow::bail!("a form needs a name: pass --name LANG=TEXT, or --file");
            }
            show(
                ctx.out,
                &api.post(&["tenants", &tenant, "forms"], &Value::Object(body))?,
            );
        }
        FormCommand::Update { id, form } => {
            let current = api.get(&["tenants", &tenant, "forms", &id])?;
            let body = form.over(as_sent(&current))?;
            show(
                ctx.out,
                &api.put(&["tenants", &tenant, "forms", &id], &Value::Object(body))?,
            );
        }
        FormCommand::Delete { id } => {
            ctx.confirm("Delete that form?")?;
            api.delete(&["tenants", &tenant, "forms", &id])?;
            notice("Form deleted.");
        }
        FormCommand::Schema { id } => {
            ctx.out
                .document(&api.get(&["tenants", &tenant, "forms", &id, "schema"])?);
        }
        FormCommand::Dcql { id } => {
            ctx.out
                .document(&api.get(&["tenants", &tenant, "forms", &id, "dcql"])?);
        }
        FormCommand::Verify { id, file } => {
            let verified = api.post(
                &["tenants", &tenant, "forms", &id, "verify"],
                &read_json(&file)?,
            )?;
            ctx.out.document(&verified);
        }
    }
    Ok(())
}

fn show(out: Output, form: &Value) {
    if out.is_json() {
        return out.document(form);
    }
    out.item(
        form,
        &[
            col("ID", "id"),
            col("Slug", "slug"),
            col("Name", "name"),
            col("Description", "description"),
            col("In use", "in_use"),
            col("Created", "created_at"),
            col("Updated", "updated_at"),
        ],
    );
    out.list(
        &form["fields"],
        &[
            col("Field", "ref"),
            col("As", "as"),
            col("Required", "required"),
            col("Help", "help"),
        ],
    );
    if form["credentials"]
        .as_array()
        .is_some_and(|c| !c.is_empty())
    {
        out.list(
            &form["credentials"],
            &[
                col("Credential", "type"),
                col("Required", "required"),
                col("Claims", "claims"),
                col("Trust", "trust"),
            ],
        );
    }
}
