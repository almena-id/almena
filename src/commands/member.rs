//! `almena member`: who belongs to the tenant, inviting more, changing roles,
//! removing people and leaving.

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
    /// Take an invitation back before it is accepted (admins).
    Uninvite {
        /// The invitation's id (`member list`).
        id: String,
    },
    /// Change a member's role (admins); a tenant keeps one admin at least.
    Update {
        /// Their user id (`member list`).
        id: String,
        /// Their new role.
        #[arg(long, value_enum)]
        role: Role,
    },
    /// Remove a member (admins); whoever signs for the tenant stays until replaced.
    Remove {
        /// Their user id (`member list`).
        id: String,
    },
    /// Leave the tenant yourself.
    Leave,
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
    col("Invitation", "invitation_id"),
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
        MemberCommand::Uninvite { id } => {
            ctx.confirm("Take that invitation back?")?;
            api.delete(&["tenants", &tenant, "invitations", &id])?;
            notice("Invitation taken back.");
        }
        MemberCommand::Update { id, role } => {
            let changed = api.patch(
                &["tenants", &tenant, "members", &id],
                &json!({"role": role.api()}),
            )?;
            ctx.out.item(&changed, COLUMNS);
        }
        MemberCommand::Remove { id } => {
            ctx.confirm("Remove that member from the tenant?")?;
            api.delete(&["tenants", &tenant, "members", &id])?;
            notice("Member removed.");
        }
        MemberCommand::Leave => {
            ctx.confirm("Leave the tenant? Coming back takes a new invitation.")?;
            let me = api.get(&["auth", "me"])?;
            let id = me["id"].as_str().unwrap_or_default();
            api.delete(&["tenants", &tenant, "members", id])?;
            notice("You left the tenant.");
        }
    }
    Ok(())
}
