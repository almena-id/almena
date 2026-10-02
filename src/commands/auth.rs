//! `almena auth`: signing in (an emailed code, the wallet, or an API token),
//! out, and who is signed in.

use std::io::{IsTerminal, Read};

use anyhow::{Context as _, bail};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::client::{ApiError, Client};
use crate::context::{Context, NotSignedIn, prompt};
use crate::credentials::{self, Kind, Stored};
use crate::output::{col, notice};
use crate::wallet;

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Sign in with a code emailed to you, your Almena wallet, or an API token.
    Login(LoginArgs),
    /// Sign out: end the sign-in and forget it (an API token is only forgotten).
    Logout,
    /// Who is signed in, to which API, and the tenant in use.
    Status,
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Your email: a six-digit code is sent to it [default: asked].
    #[arg(long, conflicts_with_all = ["wallet", "with_token"])]
    pub email: Option<String>,
    /// Sign in with your Almena wallet (a QR code to scan).
    #[arg(long, conflicts_with = "with_token")]
    pub wallet: bool,
    /// Keep an API token read from stdin (`almena token create` makes one).
    #[arg(long)]
    pub with_token: bool,
}

pub fn run(ctx: &mut Context, command: AuthCommand) -> anyhow::Result<()> {
    match command {
        AuthCommand::Login(args) => login(ctx, args),
        AuthCommand::Logout => logout(ctx),
        AuthCommand::Status => status(ctx),
    }
}

fn login(ctx: &Context, args: LoginArgs) -> anyhow::Result<()> {
    let api = ctx.public_api()?;
    let signed_in = if args.with_token {
        let token = read_token()?;
        // Who it is, before keeping it.
        let me = Client::new(&ctx.api_url, Some(token.clone()), ctx.global.verbose)?
            .get(&["auth", "me"])?;
        json!({"token": token, "expires_at": null, "user": me, "kind": "api_token"})
    } else if args.wallet {
        let request = api.post(
            &["auth", "wallet", "requests"],
            &json!({"purpose": "sign_in", "locale": ctx.locale(), "client": wallet::CLIENT}),
        )?;
        let result = wallet::wait(&api, &request, "sign in")?;
        result
            .get("session")
            .cloned()
            .filter(|s| !s.is_null())
            .context("the wallet did not sign in")?
    } else {
        let email = match args.email {
            Some(email) => email,
            None if std::io::stdin().is_terminal() => prompt("Email: ")?,
            None => bail!("pass --email, --wallet or --with-token"),
        };
        api.post(
            &["auth", "code"],
            &json!({"email": email, "locale": ctx.locale()}),
        )?;
        notice(format!("A six-digit code is on its way to {email}."));
        let code = prompt("Code: ")?;
        api.post(
            &["auth", "verify"],
            &json!({"email": email, "code": code, "locale": ctx.locale()}),
        )?
    };
    let kind = if signed_in["kind"] == "api_token" {
        Kind::ApiToken
    } else {
        Kind::Session
    };
    let stored = Stored {
        token: signed_in["token"]
            .as_str()
            .context("the API gave no token")?
            .to_owned(),
        kind,
        expires_at: signed_in["expires_at"].as_str().map(str::to_owned),
        api_url: ctx.api_url.clone(),
    };
    credentials::save(&ctx.dir, &ctx.global.profile, &stored)?;
    let user = &signed_in["user"];
    let who = user["email"]
        .as_str()
        .or(user["alias"].as_str())
        .unwrap_or("your wallet's account");
    notice(format!(
        "Signed in as {who} (profile `{}`).",
        ctx.global.profile
    ));
    Ok(())
}

fn read_token() -> anyhow::Result<String> {
    if std::io::stdin().is_terminal() {
        notice("Paste the API token, then press Enter:");
        return Ok(rpassword::read_password()?.trim().to_owned());
    }
    let mut token = String::new();
    std::io::stdin()
        .read_to_string(&mut token)
        .context("reading the token from stdin")?;
    let token = token.trim().to_owned();
    if token.is_empty() {
        bail!("no token on stdin");
    }
    Ok(token)
}

fn logout(ctx: &Context) -> anyhow::Result<()> {
    let Some(stored) = credentials::load(&ctx.dir, &ctx.global.profile)? else {
        notice("Not signed in.");
        return Ok(());
    };
    // Logging out with an API token would revoke it: it is only forgotten.
    if stored.kind == Kind::Session {
        let api = Client::new(&stored.api_url, Some(stored.token), ctx.global.verbose)?;
        match api.post(&["auth", "logout"], &Value::Null) {
            Ok(_) => {}
            // Already over on the API's side.
            Err(error)
                if error
                    .downcast_ref::<ApiError>()
                    .is_some_and(|e| e.status == 401) => {}
            Err(error) => return Err(error),
        }
    }
    credentials::delete(&ctx.dir, &ctx.global.profile)?;
    notice(format!("Signed out (profile `{}`).", ctx.global.profile));
    Ok(())
}

fn status(ctx: &Context) -> anyhow::Result<()> {
    let stored = ctx.stored()?.ok_or(NotSignedIn)?;
    let api = ctx.api()?;
    let me = api.get(&["auth", "me"])?;
    let from_env = std::env::var("ALMENA_TOKEN").is_ok_and(|t| !t.is_empty());
    let status = json!({
        "profile": ctx.global.profile,
        "api_url": ctx.api_url,
        "user": me,
        "credential": if from_env { "ALMENA_TOKEN" } else {
            match stored.kind { Kind::Session => "session", Kind::ApiToken => "api_token" }
        },
        "expires_at": stored.expires_at,
        "tenant": ctx.global.tenant.clone().or(ctx.profile().tenant),
    });
    ctx.out.item(
        &status,
        &[
            col("Profile", "profile"),
            col("API", "api_url"),
            col("Account", "user.id"),
            col("Email", "user.email"),
            col("Alias", "user.alias"),
            col("Signed in with", "credential"),
            col("Expires", "expires_at"),
            col("Tenant", "tenant"),
        ],
    );
    Ok(())
}
