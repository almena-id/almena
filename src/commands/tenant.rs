//! `almena tenant`: the tenants you belong to, their settings and health.

use clap::{Subcommand, ValueEnum};
use serde_json::{Map, Value, json};

use crate::context::{Context, resolve_tenant};
use crate::output::{Output, col, notice};

#[derive(Debug, Subcommand)]
pub enum TenantCommand {
    /// List the tenants you belong to, with your role in each.
    List,
    /// Show the tenant.
    Get,
    /// Change the tenant's name, mediator, signing flow or languages (admins).
    Update {
        /// Its new name.
        #[arg(long)]
        name: Option<String>,
        /// The mediator its identity receives messages through (see `almena mediator choices`).
        #[arg(long, conflicts_with = "no_mediator")]
        mediator: Option<String>,
        /// Remove its mediator.
        #[arg(long)]
        no_mediator: bool,
        /// Who signs as the tenant.
        #[arg(long, value_enum)]
        signing_flow: Option<SigningFlow>,
        /// The member who signs, under `single-user` (their user id; see `almena member list`).
        #[arg(long)]
        signer: Option<String>,
        /// The languages it works in, of the platform's: all of them, repeated or
        /// comma-separated (`--language en,es`); the trust anchor's are all.
        #[arg(long = "language", value_delimiter = ',')]
        languages: Vec<String>,
    },
    /// What the tenant still needs set up to operate.
    Health,
    /// Work in this tenant from now on (by id or name), in this profile.
    Use {
        /// The tenant's id or name.
        tenant: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SigningFlow {
    /// Any of its admins.
    AnyAdmin,
    /// One member, named with --signer.
    SingleUser,
}

impl SigningFlow {
    fn api(self) -> &'static str {
        match self {
            Self::AnyAdmin => "any_admin",
            Self::SingleUser => "single_user",
        }
    }
}

pub fn run(ctx: &mut Context, command: TenantCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    match command {
        TenantCommand::List => {
            let tenants = api.get(&["tenants"])?;
            ctx.out.list(
                &tenants,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("Role", "role"),
                    col("Signs", "signs"),
                    col("Created", "created_at"),
                ],
            );
        }
        TenantCommand::Get => {
            let tenant = ctx.tenant(&api)?;
            show(ctx.out, &api.get(&["tenants", &tenant])?);
        }
        TenantCommand::Update {
            name,
            mediator,
            no_mediator,
            signing_flow,
            signer,
            languages,
        } => {
            let tenant = ctx.tenant(&api)?;
            let mut body = Map::new();
            if let Some(name) = name {
                body.insert("name".into(), json!(name));
            }
            if let Some(mediator) = mediator {
                body.insert("mediator_id".into(), json!(mediator));
            } else if no_mediator {
                body.insert("mediator_id".into(), Value::Null);
            }
            if let Some(flow) = signing_flow {
                body.insert("signing_flow".into(), json!(flow.api()));
            }
            if let Some(signer) = signer {
                body.insert("signer_id".into(), json!(signer));
            }
            if !languages.is_empty() {
                body.insert("languages".into(), json!(languages));
            }
            if body.is_empty() {
                anyhow::bail!(
                    "nothing to change: pass --name, --mediator, --signing-flow, --signer or --language"
                );
            }
            show(
                ctx.out,
                &api.patch(&["tenants", &tenant], &Value::Object(body))?,
            );
        }
        TenantCommand::Health => {
            let tenant = ctx.tenant(&api)?;
            let health = api.get(&["tenants", &tenant, "health"])?;
            if !ctx.out.is_json() {
                notice(format!("Health: {}%", ctx.out.cell(&health["score"])));
            }
            if ctx.out.is_json() {
                ctx.out.document(&health);
            } else {
                ctx.out.list(
                    &health["checks"],
                    &[
                        col("Check", "check"),
                        col("Done", "done"),
                        col("Issue", "issue"),
                    ],
                );
            }
        }
        TenantCommand::Use { tenant } => {
            let id = resolve_tenant(&api, &tenant)?;
            ctx.profile_mut().tenant = Some(id.clone());
            ctx.save_config()?;
            notice(format!(
                "Working in tenant {id} (profile `{}`).",
                ctx.global.profile
            ));
        }
    }
    Ok(())
}

fn show(out: Output, tenant: &Value) {
    out.item(
        tenant,
        &[
            col("ID", "id"),
            col("Name", "name"),
            col("Your role", "role"),
            col("You sign", "signs"),
            col("Identity", "identity.name"),
            col("Mediator", "mediator.name"),
            col("Signing flow", "signing_flow"),
            col("Signer", "signer.email"),
            col("Languages", "languages"),
            col("Created", "created_at"),
        ],
    );
}
