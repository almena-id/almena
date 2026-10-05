//! One module per resource, each with its subcommands and a `run`; this one
//! dispatches the parsed command line, and holds what several share.

pub mod account;
pub mod agent;
pub mod application;
pub mod auth;
pub mod catalog;
pub mod category;
mod completions;
pub mod config;
pub mod credential_type;
mod described;
pub mod domain;
pub mod field;
pub mod form;
pub mod identity;
pub mod issuance;
pub mod issuer;
pub mod mediator;
pub mod member;
pub mod signature;
pub mod subscription;
pub mod tenant;
pub mod token;
pub mod value_domain;
pub mod verifier;

use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context as _, bail};
use clap::Args;
use serde_json::{Map, Value};

use crate::cli::{Cli, Command};
use crate::client::Client;
use crate::context::Context;

pub fn run(cli: Cli) -> anyhow::Result<()> {
    if let Command::Completions { shell } = cli.command {
        return completions::run(shell);
    }
    let mut ctx = Context::new(cli.global)?;
    match cli.command {
        Command::Auth(command) => auth::run(&mut ctx, command),
        Command::Account(command) => account::run(&ctx, command),
        Command::Token(command) => token::run(&ctx, command),
        Command::Tenant(command) => tenant::run(&mut ctx, command),
        Command::Issuer(command) => issuer::run(&ctx, command),
        Command::Verifier(command) => verifier::run(&ctx, command),
        Command::Mediator(command) => mediator::run(&ctx, command),
        Command::Identity(command) => identity::run(&ctx, command),
        Command::Signature(command) => signature::run(&ctx, command),
        Command::Domain(command) => domain::run(&ctx, command),
        Command::Member(command) => member::run(&ctx, command),
        Command::Form(command) => form::run(&ctx, command),
        Command::Field(command) => field::run(&ctx, command),
        Command::Category(command) => category::run(&ctx, command),
        Command::ValueDomain(command) => value_domain::run(&ctx, command),
        Command::Subscription(command) => subscription::run(&ctx, command),
        Command::CredentialType(command) => credential_type::run(&ctx, command),
        Command::Catalog(command) => catalog::run(&ctx, command),
        Command::Application(command) => application::run(&ctx, command),
        Command::Issuance(command) => issuance::run(&ctx, command),
        Command::Agent(command) => agent::run(&ctx, command),
        Command::Config(command) => config::run(&mut ctx, command),
        Command::Completions { .. } => unreachable!("handled above"),
    }
}

/// Paging of a list: one page, or every page.
#[derive(Debug, Args)]
pub struct PageArgs {
    /// Items per page (1–100).
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
    pub limit: u16,
    /// Start after this cursor (a page's `next_cursor`).
    #[arg(long, conflicts_with = "all")]
    pub cursor: Option<String>,
    /// Follow `next_cursor` until the last page.
    #[arg(long)]
    pub all: bool,
}

/// A paged list (`{items, next_cursor}`): its items, every page's with `--all`.
/// With one page, says how to get the next.
pub fn paged(api: &Client, segments: &[&str], page: &PageArgs) -> anyhow::Result<Value> {
    paged_with(api, segments, page, &[])
}

/// [`paged`], with more query parameters (filters) on every page.
pub fn paged_with(
    api: &Client,
    segments: &[&str],
    page: &PageArgs,
    filters: &[(&str, String)],
) -> anyhow::Result<Value> {
    let mut items = Vec::new();
    let mut cursor = page.cursor.clone();
    loop {
        let mut query = vec![("limit", page.limit.to_string())];
        query.extend(filters.iter().cloned());
        if let Some(cursor) = &cursor {
            query.push(("cursor", cursor.clone()));
        }
        let mut answer = api.get_query(segments, &query)?;
        if let Some(Value::Array(found)) = answer.get_mut("items").map(Value::take) {
            items.extend(found);
        }
        cursor = answer["next_cursor"].as_str().map(str::to_owned);
        match &cursor {
            Some(next) if !page.all => {
                crate::output::notice(format!("More with --cursor {next} (or --all)."));
                break;
            }
            Some(_) => {}
            None => break,
        }
    }
    Ok(Value::Array(items))
}

/// `lang=text`, for texts given per language.
pub fn parse_text(raw: &str) -> Result<(String, String), String> {
    let (language, text) = raw
        .split_once('=')
        .ok_or_else(|| format!("expected LANG=TEXT, got `{raw}`"))?;
    let language = language.trim();
    if language.len() != 2 || !language.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(format!("`{language}` is not a two-letter language code"));
    }
    Ok((language.to_owned(), text.to_owned()))
}

/// Texts per language, as the API takes them: `{"en": …, "es": …}`.
pub fn texts(pairs: &[(String, String)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .map(|(lang, text)| (lang.clone(), Value::String(text.clone())))
            .collect(),
    )
}

/// A JSON object read from a file, or from stdin for `-`.
pub fn read_json(path: &Path) -> anyhow::Result<Value> {
    let text = if path == Path::new("-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .context("reading stdin")?;
        text
    } else {
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
    };
    serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))
}

/// The body read from `--file` (an object), or an empty one.
pub fn body_from(file: Option<&Path>) -> anyhow::Result<Map<String, Value>> {
    match file.map(read_json).transpose()? {
        None => Ok(Map::new()),
        Some(Value::Object(body)) => Ok(body),
        Some(_) => bail!("the file must hold a JSON object"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_are_given_per_language() {
        assert_eq!(
            parse_text("en=Hello = world"),
            Ok(("en".into(), "Hello = world".into()))
        );
        assert!(parse_text("Hello").is_err());
        assert!(parse_text("eng=Hello").is_err());
        let pairs = vec![
            ("en".to_owned(), "A".to_owned()),
            ("es".to_owned(), "B".to_owned()),
        ];
        assert_eq!(texts(&pairs), serde_json::json!({"en": "A", "es": "B"}));
    }
}
