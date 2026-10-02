//! `almena config`: the profile's settings, kept in `config.toml`.

use anyhow::bail;
use clap::{Subcommand, ValueEnum};
use serde_json::json;

use crate::cli::{Locale, OutputFormat};
use crate::config::Profile;
use crate::context::Context;
use crate::output::{col, notice};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Show the profile's settings, and those in effect.
    Show,
    /// Set one of the profile's settings.
    Set {
        /// The setting.
        #[arg(value_enum)]
        key: Key,
        /// Its value.
        value: String,
    },
    /// Remove one of the profile's settings (back to its default).
    Unset {
        /// The setting.
        #[arg(value_enum)]
        key: Key,
    },
    /// Where the configuration is kept.
    Path,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Key {
    /// The Almena API.
    ApiUrl,
    /// The Almena agent.
    AgentUrl,
    /// The tenant to work in, by id (`almena tenant use` sets it by name too).
    Tenant,
    /// `table` or `json`.
    Output,
    /// `en` or `es`.
    Locale,
}

pub fn run(ctx: &mut Context, command: ConfigCommand) -> anyhow::Result<()> {
    match command {
        ConfigCommand::Show => {
            let profile = ctx.profile();
            let shown = json!({
                "profile": ctx.global.profile,
                "api_url": ctx.api_url,
                "agent_url": ctx.agent_url,
                "tenant": ctx.global.tenant.clone().or(profile.tenant),
                "output": ctx.out.format,
                "locale": ctx.out.locale,
                "saved": serde_json::to_value(ctx.profile())?,
            });
            ctx.out.item(
                &shown,
                &[
                    col("Profile", "profile"),
                    col("API", "api_url"),
                    col("Agent", "agent_url"),
                    col("Tenant", "tenant"),
                    col("Output", "output"),
                    col("Locale", "locale"),
                ],
            );
        }
        ConfigCommand::Set { key, value } => {
            set(ctx.profile_mut(), key, Some(&value))?;
            ctx.save_config()?;
            notice("Saved.");
        }
        ConfigCommand::Unset { key } => {
            set(ctx.profile_mut(), key, None)?;
            ctx.save_config()?;
            notice("Removed.");
        }
        ConfigCommand::Path => println!("{}", ctx.dir.join("config.toml").display()),
    }
    Ok(())
}

fn set(profile: &mut Profile, key: Key, value: Option<&str>) -> anyhow::Result<()> {
    match key {
        Key::ApiUrl => profile.api_url = value.map(url).transpose()?,
        Key::AgentUrl => profile.agent_url = value.map(url).transpose()?,
        Key::Tenant => profile.tenant = value.map(str::to_owned),
        Key::Output => {
            profile.output = value
                .map(|v| OutputFormat::from_str(v, true))
                .transpose()
                .map_err(anyhow::Error::msg)?;
        }
        Key::Locale => {
            profile.locale = value
                .map(|v| Locale::from_str(v, true))
                .transpose()
                .map_err(anyhow::Error::msg)?;
        }
    }
    Ok(())
}

fn url(value: &str) -> anyhow::Result<String> {
    match url::Url::parse(value) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {
            Ok(value.trim_end_matches('/').to_owned())
        }
        _ => bail!("`{value}` is not an http(s) URL"),
    }
}
