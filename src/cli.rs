//! Command-line surface: global options and the subcommand tree. Each
//! resource's subcommands live beside its code, in `commands/<resource>.rs`.

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};

use crate::commands::{
    account, agent, application, auth, catalog, category, config, credential_type, domain, field,
    form, identity, issuance, issuer, mediator, member, signature, subscription, tenant, token,
    value_domain, verifier,
};

/// The CLI's version: `ALMENA_VERSION` when the build sets it (the release's
/// `year.month.sequence`, see `.github/workflows/release.yml`), else the crate
/// version.
pub const VERSION: &str = match option_env!("ALMENA_VERSION") {
    Some(version) if !version.is_empty() => version,
    _ => env!("CARGO_PKG_VERSION"),
};

#[derive(Debug, Parser)]
#[command(name = "almena", version = VERSION, about, long_about = None, propagate_version = true)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

/// Options accepted by every subcommand. Unset, each falls back to the
/// profile's value (`almena config`), then to its default.
#[derive(Debug, Clone, Args)]
pub struct GlobalArgs {
    /// Print more detail about what the command is doing.
    #[arg(short, long, global = true, env = "ALMENA_VERBOSE")]
    pub verbose: bool,

    /// Configuration profile: its settings and its sign-in.
    #[arg(long, global = true, env = "ALMENA_PROFILE", default_value = "default")]
    pub profile: String,

    /// The Almena API [default: https://api.almena.id].
    #[arg(long, global = true, env = "ALMENA_API_URL", value_name = "URL")]
    pub api_url: Option<String>,

    /// The Almena agent [default: https://agent.almena.id].
    #[arg(long, global = true, env = "ALMENA_AGENT_URL", value_name = "URL")]
    pub agent_url: Option<String>,

    /// Tenant to work in, by id or name [default: the profile's, or your only one].
    #[arg(long, global = true, env = "ALMENA_TENANT")]
    pub tenant: Option<String>,

    /// How results are printed [default: table].
    #[arg(short, long, global = true, env = "ALMENA_OUTPUT", value_enum)]
    pub output: Option<OutputFormat>,

    /// Language of emails, of the wallet's sheet and of texts shown [default: en].
    #[arg(long, global = true, env = "ALMENA_LOCALE", value_enum)]
    pub locale: Option<Locale>,

    /// Answer yes to every confirmation.
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable tables.
    Table,
    /// The API's answer, as it came.
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    En,
    Es,
}

impl Locale {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Es => "es",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sign in and out.
    #[command(subcommand)]
    Auth(auth::AuthCommand),
    /// Your account: alias and ways in.
    #[command(subcommand)]
    Account(account::AccountCommand),
    /// API tokens for scripts and CI.
    #[command(subcommand)]
    Token(token::TokenCommand),
    /// Tenants you belong to.
    #[command(subcommand)]
    Tenant(tenant::TenantCommand),
    /// The tenant's issuers.
    #[command(subcommand)]
    Issuer(issuer::IssuerCommand),
    /// The tenant's verifiers.
    #[command(subcommand)]
    Verifier(verifier::VerifierCommand),
    /// The tenant's mediators.
    #[command(subcommand)]
    Mediator(mediator::MediatorCommand),
    /// The tenant's identities (its register of DIDs).
    #[command(subcommand)]
    Identity(identity::IdentityCommand),
    /// Identities whose DID waits for a signature.
    #[command(subcommand)]
    Signature(signature::SignatureCommand),
    /// The tenant's linked domains.
    #[command(subcommand)]
    Domain(domain::DomainCommand),
    /// The tenant's members and invitations.
    #[command(subcommand)]
    Member(member::MemberCommand),
    /// The tenant's subscription; the trust anchor's admins: every account's.
    #[command(subcommand)]
    Subscription(subscription::SubscriptionCommand),
    /// The tenant's forms.
    #[command(subcommand)]
    Form(form::FormCommand),
    /// The tenant's own fields, beside Almena's catalogue (the trust anchor's: the catalogue).
    #[command(subcommand)]
    Field(field::FieldCommand),
    /// The tenant's own credential types (the trust anchor's: Almena's catalogue).
    #[command(subcommand)]
    CredentialType(credential_type::CredentialTypeCommand),
    /// The trust anchor's categories for fields and credential types.
    #[command(subcommand)]
    Category(category::CategoryCommand),
    /// The trust anchor's value lists, which coded fields draw on.
    #[command(subcommand)]
    ValueDomain(value_domain::ValueDomainCommand),
    /// Almena's public catalogues: fields, credential types and what is published.
    #[command(subcommand)]
    Catalog(catalog::CatalogCommand),
    /// Applications the tenant's issuers received.
    #[command(subcommand)]
    Application(application::ApplicationCommand),
    /// Issuing an accepted application's credential.
    #[command(subcommand)]
    Issuance(issuance::IssuanceCommand),
    /// Talk to the Almena agent.
    #[command(subcommand)]
    Agent(agent::AgentCommand),
    /// The profile's settings.
    #[command(subcommand)]
    Config(config::ConfigCommand),
    /// Print the shell completion script for `almena`.
    Completions {
        /// Shell to generate the script for.
        shell: Shell,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
