//! `almena form`: the tenant's forms — what is asked of a person, whoever asks
//! it — and verifying credentials presented for one.

use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::commands::{body_from, parse_text, read_json, texts};
use crate::context::Context;
use crate::output::{Output, col};

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
    Create {
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
                    col("Updated", "updated_at"),
                ],
            );
        }
        FormCommand::Get { id } => show(ctx.out, &api.get(&["tenants", &tenant, "forms", &id])?),
        FormCommand::Create {
            file,
            name,
            description,
            field,
            optional_field,
            credential,
        } => {
            let mut body = body_from(file.as_deref())?;
            if !name.is_empty() {
                body.insert("name".into(), texts(&name));
            }
            if !description.is_empty() {
                body.insert("description".into(), texts(&description));
            }
            if !field.is_empty() || !optional_field.is_empty() {
                let fields = field
                    .iter()
                    .map(|r| json!({"ref": r, "required": true}))
                    .chain(
                        optional_field
                            .iter()
                            .map(|r| json!({"ref": r, "required": false})),
                    )
                    .collect();
                body.insert("fields".into(), Value::Array(fields));
            }
            if !credential.is_empty() {
                let wanted = credential
                    .iter()
                    .map(|kind| json!({"type": kind}))
                    .collect();
                body.insert("credentials".into(), Value::Array(wanted));
            }
            if !body.contains_key("name") {
                anyhow::bail!("a form needs a name: pass --name LANG=TEXT, or --file");
            }
            show(
                ctx.out,
                &api.post(&["tenants", &tenant, "forms"], &Value::Object(body))?,
            );
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
