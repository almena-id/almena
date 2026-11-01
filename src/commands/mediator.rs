//! `almena mediator`: the tenant's mediators, those it may pick, and whether
//! one is up.

use std::time::Duration;

use anyhow::{Context as _, bail};
use clap::{ArgAction, Subcommand};
use serde_json::{Map, Value, json};

use crate::cli::VERSION;
use crate::client::ApiError;
use crate::commands::described::{self, Kind};
use crate::commands::{PageArgs, paged};
use crate::context::Context;
use crate::output::{Output, col, notice};

#[derive(Debug, Subcommand)]
pub enum MediatorCommand {
    /// List the tenant's mediators, newest first.
    List(PageArgs),
    /// Show one, with its DID.
    Get {
        /// Its id.
        id: String,
    },
    /// Register a mediator listening on a subdomain of one of the tenant's verified domains.
    Create {
        /// Its name (1–200 characters).
        #[arg(long)]
        name: String,
        /// Where it listens: one label or more (`relay`, `eu.relay`) under --domain.
        #[arg(long)]
        subdomain: String,
        /// The verified domain it listens under, by id (see `almena domain list`).
        #[arg(long)]
        domain: String,
        /// Offer it to every tenant once published.
        #[arg(long)]
        public: bool,
    },
    /// Change its name, address or whether it is offered to every tenant.
    Update {
        /// Its id.
        id: String,
        /// Its new name.
        #[arg(long)]
        name: Option<String>,
        /// Move it to this subdomain (`relay`, `eu.relay`) of --domain.
        #[arg(long, requires = "domain")]
        subdomain: Option<String>,
        /// The verified domain it moves under, by id (see `almena domain list`).
        #[arg(long, requires = "subdomain")]
        domain: Option<String>,
        /// Offer it to every tenant (true) or not (false).
        #[arg(long, action = ArgAction::Set, value_name = "BOOL")]
        public: Option<bool>,
    },
    /// Delete it and its identity (admins).
    Delete {
        /// Its id.
        id: String,
    },
    /// Publish it: its DID resolves and the catalogue lists it (admins).
    Publish {
        /// Its id.
        id: String,
    },
    /// Take it back to a draft (admins).
    Unpublish {
        /// Its id.
        id: String,
    },
    /// The mediators the tenant, its issuers and verifiers may pick.
    Choices,
    /// Check a mediator is up: its `/health`, its DID document and its invitation;
    /// fails when any of them does.
    Health {
        /// The mediator's id (the tenant's, or one of `almena mediator choices`), or
        /// its URL (`https://…`, no sign-in needed).
        mediator: String,
    },
}

pub fn run(ctx: &Context, command: MediatorCommand) -> anyhow::Result<()> {
    if let MediatorCommand::Health { mediator } = command {
        return health(ctx, &mediator);
    }
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        MediatorCommand::List(page) => {
            let items = paged(&api, &["tenants", &tenant, "mediators"], &page)?;
            ctx.out.list(
                &items,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("URL", "url"),
                    col("Public", "public"),
                    col("Signature", "signature"),
                    col("Published", "published_at"),
                ],
            );
        }
        MediatorCommand::Get { id } => {
            show(ctx.out, &api.get(&["tenants", &tenant, "mediators", &id])?)
        }
        MediatorCommand::Create {
            name,
            subdomain,
            domain,
            public,
        } => {
            let made = api.post(
                &["tenants", &tenant, "mediators"],
                &json!({"name": name, "subdomain": subdomain, "domain_id": domain, "public": public}),
            )?;
            show(ctx.out, &made);
        }
        MediatorCommand::Update {
            id,
            name,
            subdomain,
            domain,
            public,
        } => {
            let mut body = Map::new();
            if let Some(name) = name {
                body.insert("name".into(), json!(name));
            }
            if let (Some(subdomain), Some(domain)) = (subdomain, domain) {
                body.insert("subdomain".into(), json!(subdomain));
                body.insert("domain_id".into(), json!(domain));
            }
            if let Some(public) = public {
                body.insert("public".into(), json!(public));
            }
            if body.is_empty() {
                anyhow::bail!(
                    "nothing to change: pass --name, --subdomain with --domain, or --public"
                );
            }
            show(
                ctx.out,
                &api.patch(
                    &["tenants", &tenant, "mediators", &id],
                    &Value::Object(body),
                )?,
            );
        }
        MediatorCommand::Delete { id } => {
            described::delete(ctx, &api, &tenant, Kind::Mediator, &id)?
        }
        MediatorCommand::Publish { id } => {
            described::publish(ctx, &api, &tenant, Kind::Mediator, &id, show)?;
        }
        MediatorCommand::Unpublish { id } => {
            described::unpublish(ctx, &api, &tenant, Kind::Mediator, &id, show)?;
        }
        MediatorCommand::Choices => {
            let choices = api.get(&["tenants", &tenant, "mediator-choices"])?;
            ctx.out.list(
                &choices,
                &[
                    col("ID", "id"),
                    col("Name", "name"),
                    col("URL", "url"),
                    col("Own", "own"),
                    col("Published", "published"),
                ],
            );
            if !ctx.out.is_json() {
                notice("Pick one with --mediator on `tenant update`, `issuer create`…");
            }
        }
        MediatorCommand::Health { .. } => unreachable!("handled before signing in"),
    }
    Ok(())
}

/// Fetches the mediator's `/health`, `/.well-known/did.json` and
/// `/oob/invitation`, prints a row for each and fails if any failed.
fn health(ctx: &Context, mediator: &str) -> anyhow::Result<()> {
    let base = if mediator.starts_with("https://") || mediator.starts_with("http://") {
        mediator.to_owned()
    } else {
        mediator_url(ctx, mediator)?
    };
    let mut base = url::Url::parse(&base).with_context(|| format!("not a URL: {base}"))?;
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    let http = reqwest::blocking::Client::builder()
        .user_agent(format!("almena/{VERSION}"))
        .timeout(Duration::from_secs(10))
        .build()?;
    let fetch = |path: &str| -> (String, Result<(u16, Value), String>) {
        let url = base
            .join(path)
            .map_or_else(|_| path.to_owned(), String::from);
        if ctx.global.verbose {
            eprintln!("> GET {url}");
        }
        let answer = http
            .get(&url)
            .send()
            .map_err(|error| format!("unreachable: {error}"))
            .map(|response| {
                let status = response.status().as_u16();
                (status, response.json().unwrap_or(Value::Null))
            });
        (url, answer)
    };

    let mut rows = Vec::new();
    let (url, answer) = fetch("health");
    let did = answer
        .as_ref()
        .ok()
        .and_then(|(_, body)| body["did"].as_str())
        .map(str::to_owned);
    rows.push(row("health", url, answer, |status, body| {
        let state = body["status"].as_str().unwrap_or("-");
        let detail = format!(
            "{state}, version {}, capacity {}",
            body["version"].as_str().unwrap_or("-"),
            body["capacity"].as_str().unwrap_or("-"),
        );
        (status == 200 && state == "ok", detail)
    }));

    let (url, answer) = fetch(".well-known/did.json");
    rows.push(row("did document", url, answer, |status, body| {
        let id = body["id"].as_str().unwrap_or_default();
        match &did {
            _ if status != 200 || id.is_empty() => (false, "no DID document".to_owned()),
            Some(did) if did != id => (false, format!("{id}, but /health says {did}")),
            _ => (true, id.to_owned()),
        }
    }));

    let (url, answer) = fetch("oob/invitation");
    if matches!(answer, Ok((404, _))) {
        rows.push(json!({"check": "invitation", "url": url, "status": 404,
            "ok": true, "detail": "not served"}));
    } else {
        rows.push(row("invitation", url, answer, |status, body| {
            let from = body["invitation"]["from"].as_str().unwrap_or_default();
            match &did {
                _ if status != 200 || from.is_empty() => (false, "no invitation".to_owned()),
                Some(did) if did != from => (false, format!("from {from}, not {did}")),
                _ => (true, format!("from {from}")),
            }
        }));
    }

    let failed = rows.iter().filter(|r| r["ok"] != json!(true)).count();
    let rows = Value::Array(rows);
    ctx.out.list(
        &rows,
        &[
            col("Check", "check"),
            col("URL", "url"),
            col("HTTP", "status"),
            col("OK", "ok"),
            col("Detail", "detail"),
        ],
    );
    if failed > 0 {
        bail!("{failed} of 3 checks failed for {base}");
    }
    Ok(())
}

/// One check's row: the answer judged by `judge`, or why there was none.
fn row(
    check: &str,
    url: String,
    answer: Result<(u16, Value), String>,
    judge: impl FnOnce(u16, &Value) -> (bool, String),
) -> Value {
    match answer {
        Ok((status, body)) => {
            let (ok, detail) = judge(status, &body);
            json!({"check": check, "url": url, "status": status, "ok": ok, "detail": detail})
        }
        Err(error) => {
            json!({"check": check, "url": url, "status": null, "ok": false, "detail": error})
        }
    }
}

/// The URL of a mediator by id: the tenant's own, else one it may pick.
fn mediator_url(ctx: &Context, id: &str) -> anyhow::Result<String> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    let found = match api.get(&["tenants", &tenant, "mediators", id]) {
        Ok(own) => own,
        Err(error)
            if error
                .downcast_ref::<ApiError>()
                .is_some_and(|e| e.status == 404) =>
        {
            let choices = api.get(&["tenants", &tenant, "mediator-choices"])?;
            choices
                .as_array()
                .and_then(|all| all.iter().find(|m| m["id"].as_str() == Some(id)).cloned())
                .ok_or(error)?
        }
        Err(error) => return Err(error),
    };
    found["url"]
        .as_str()
        .filter(|url| !url.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("mediator {id} has no URL"))
}

fn show(out: Output, mediator: &Value) {
    out.item(
        mediator,
        &[
            col("ID", "id"),
            col("Name", "name"),
            col("URL", "url"),
            col("Public", "public"),
            col("DID", "did"),
            col("Signature", "signature"),
            col("Published", "published_at"),
            col("Created", "created_at"),
        ],
    );
}
