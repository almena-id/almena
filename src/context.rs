//! What every command works with: the global options settled against the
//! profile, the sign-in, an API client and the tenant to work in.

use std::fmt;
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use anyhow::{Context as _, bail};
use serde_json::Value;

use crate::cli::{GlobalArgs, Locale, OutputFormat};
use crate::client::Client;
use crate::config::{self, Config, Profile};
use crate::credentials::{self, Stored};
use crate::output::Output;

pub const DEFAULT_API_URL: &str = "https://api.almena.id";
pub const DEFAULT_AGENT_URL: &str = "https://agent.almena.id";

/// No sign-in for this profile (exit status 3).
#[derive(Debug)]
pub struct NotSignedIn;

impl fmt::Display for NotSignedIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not signed in: run `almena auth login`, or set ALMENA_TOKEN")
    }
}

impl std::error::Error for NotSignedIn {}

pub struct Context {
    pub global: GlobalArgs,
    pub dir: PathBuf,
    pub config: Config,
    pub api_url: String,
    pub agent_url: String,
    pub out: Output,
}

impl Context {
    pub fn new(global: GlobalArgs) -> anyhow::Result<Self> {
        let dir = config::dir()?;
        let config = config::load(&dir)?;
        let profile = config
            .profiles
            .get(&global.profile)
            .cloned()
            .unwrap_or_default();
        let api_url = global
            .api_url
            .clone()
            .or(profile.api_url)
            .unwrap_or_else(|| DEFAULT_API_URL.to_owned());
        let agent_url = global
            .agent_url
            .clone()
            .or(profile.agent_url)
            .unwrap_or_else(|| DEFAULT_AGENT_URL.to_owned());
        let out = Output {
            format: global
                .output
                .or(profile.output)
                .unwrap_or(OutputFormat::Table),
            locale: global.locale.or(profile.locale).unwrap_or(Locale::En),
        };
        Ok(Self {
            global,
            dir,
            config,
            api_url,
            agent_url,
            out,
        })
    }

    pub fn profile(&self) -> Profile {
        self.config
            .profiles
            .get(&self.global.profile)
            .cloned()
            .unwrap_or_default()
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        self.config
            .profiles
            .entry(self.global.profile.clone())
            .or_default()
    }

    pub fn save_config(&self) -> anyhow::Result<()> {
        config::save(&self.dir, &self.config)
    }

    pub fn locale(&self) -> &'static str {
        self.out.locale.code()
    }

    /// The sign-in in use: `ALMENA_TOKEN`, else the profile's.
    pub fn stored(&self) -> anyhow::Result<Option<Stored>> {
        if let Some(token) = std::env::var("ALMENA_TOKEN").ok().filter(|t| !t.is_empty()) {
            return Ok(Some(Stored {
                token,
                kind: credentials::Kind::ApiToken,
                expires_at: None,
                api_url: self.api_url.clone(),
            }));
        }
        credentials::load(&self.dir, &self.global.profile)
    }

    /// A client for the public endpoints: no sign-in sent.
    pub fn public_api(&self) -> anyhow::Result<Client> {
        Client::new(&self.api_url, None, self.global.verbose)
    }

    /// A client signed in as the profile (or `ALMENA_TOKEN`).
    pub fn api(&self) -> anyhow::Result<Client> {
        let stored = self.stored()?.ok_or(NotSignedIn)?;
        if stored.api_url.trim_end_matches('/') != self.api_url.trim_end_matches('/') {
            bail!(
                "the sign-in of profile `{}` is for {}, not {}: run `almena auth login`",
                self.global.profile,
                stored.api_url,
                self.api_url
            );
        }
        Client::new(&self.api_url, Some(stored.token), self.global.verbose)
    }

    /// The tenant to work in, as an id: `--tenant` (an id or a name), the
    /// profile's, or the only one there is.
    pub fn tenant(&self, api: &Client) -> anyhow::Result<String> {
        if let Some(wanted) = &self.global.tenant {
            return resolve_tenant(api, wanted);
        }
        if let Some(id) = self.profile().tenant {
            return Ok(id);
        }
        let tenants = api.get(&["tenants"])?;
        match tenants.as_array().map(Vec::as_slice).unwrap_or_default() {
            [only] => Ok(only["id"].as_str().unwrap_or_default().to_owned()),
            [] => bail!("you belong to no tenant"),
            _ => bail!(
                "you belong to several tenants: pass --tenant, or pick one with `almena tenant use`"
            ),
        }
    }

    /// Asks before something that cannot be undone, unless `--yes`.
    pub fn confirm(&self, question: &str) -> anyhow::Result<()> {
        if self.global.yes {
            return Ok(());
        }
        if !std::io::stdin().is_terminal() {
            bail!("{question} Pass --yes to confirm without asking.");
        }
        let answer = prompt(&format!("{question} [y/N] "))?;
        if matches!(answer.to_lowercase().as_str(), "y" | "yes") {
            Ok(())
        } else {
            bail!("cancelled")
        }
    }
}

/// A tenant named by id, or by name among the account's tenants.
pub fn resolve_tenant(api: &Client, wanted: &str) -> anyhow::Result<String> {
    let tenants = api.get(&["tenants"])?;
    let tenants = tenants.as_array().map(Vec::as_slice).unwrap_or_default();
    let id = |tenant: &Value| tenant["id"].as_str().unwrap_or_default().to_owned();
    if let Some(found) = tenants.iter().find(|t| t["id"].as_str() == Some(wanted)) {
        return Ok(id(found));
    }
    let named: Vec<&Value> = tenants
        .iter()
        .filter(|t| t["name"].as_str() == Some(wanted))
        .collect();
    match named.as_slice() {
        [one] => Ok(id(one)),
        [] => bail!("no tenant of yours is `{wanted}`; see `almena tenant list`"),
        _ => bail!("several tenants are named `{wanted}`: use its id"),
    }
}

/// A line typed at the terminal.
pub fn prompt(question: &str) -> anyhow::Result<String> {
    eprint!("{question}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("reading the answer")?;
    Ok(line.trim().to_owned())
}
