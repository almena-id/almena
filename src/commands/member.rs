//! `almena member`: who belongs to the tenant, and inviting more.

use clap::{Subcommand, ValueEnum};
use serde_json::json;

use crate::context::Context;
use crate::output::{col, notice};

#[derive(Debug, Subcommand)]
pub enum MemberCommand {
    /// List members and pending invitations.
    List,
    /// Invite someone by email (admins); they join when they next sign in with it.
    Invite {
        /// Their email.
        #[arg(long)]
        email: String,
        /// Their role in the tenant.
        #[arg(long, value_enum, default_value_t = Role::Member)]
        role: Role,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Role {
    Admin,
    Member,
}

impl Role {
    fn api(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

const COLUMNS: &[crate::output::Column] = &[
    col("User", "user_id"),
    col("Email", "email"),
    col("Alias", "alias"),
    col("Role", "role"),
    col("Status", "status"),
    col("Wallet", "wallet"),
    col("Since", "since"),
];

pub fn run(ctx: &Context, command: MemberCommand) -> anyhow::Result<()> {
    let api = ctx.api()?;
    let tenant = ctx.tenant(&api)?;
    match command {
        MemberCommand::List => ctx
            .out
            .list(&api.get(&["tenants", &tenant, "members"])?, COLUMNS),
        MemberCommand::Invite { email, role } => {
            let invited = api.post(
                &["tenants", &tenant, "invitations"],
                &json!({"email": email, "role": role.api(), "locale": ctx.locale()}),
            )?;
            ctx.out.item(&invited, COLUMNS);
            notice(format!("Invited {email}."));
        }
    }
    Ok(())
}
