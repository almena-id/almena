# almena

Command-line client of the Almena ID platform. The binary is `almena`.

## Install

With [Homebrew](https://brew.sh), on macOS (Apple silicon) or Linux (x86_64):

```bash
brew install almena-id/tap/almena
```

It installs the shell completions for bash, zsh and fish as well, and
`brew upgrade` brings the newest version distributed.

With [winget](https://learn.microsoft.com/windows/package-manager/), on
Windows (x86_64):

```powershell
winget install AlmenaID.Almena
```

`winget upgrade AlmenaID.Almena` brings the newest version distributed, once
Microsoft has merged it into its catalogue. Or with [Scoop](https://scoop.sh):

```powershell
scoop bucket add almena https://github.com/almena-id/scoop-bucket
scoop install almena/almena
```

`scoop update almena` brings the newest version distributed. For tab
completion in PowerShell, add this line to your `$PROFILE`:
`almena completions powershell | Out-String | Invoke-Expression`.

Every merge into `main` publishes a [release](https://github.com/almena-id/almena/releases)
with prebuilt binaries for Linux (x86_64, Ubuntu 24.04 or later), macOS (Apple
silicon) and Windows (x86_64), plus `SHA256SUMS`. Versions are
`year.month.sequence` (e.g. `2026.10.1`); `almena --version` reports it.

Each binary is signed: on macOS with Developer ID and notarized by Apple, on
Windows with Authenticode; `SHA256SUMS.asc` signs the checksums of all of them
with the Almena release key. To check a download (Linux especially, which has
no system-level signature):

```bash
gpg --verify SHA256SUMS.asc SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
```

## Build

Requires a Rust toolchain (edition 2024) and [Task](https://taskfile.dev).

```sh
task build      # target/release/almena
task install    # into ~/.cargo/bin
task run -- --help
```

## Signing in

```sh
almena auth login                      # a six-digit code emailed to you
almena auth login --wallet             # scan a QR code with your Almena wallet
almena token create --name ci          # an API token for scripts and CI…
ALMENA_TOKEN=almena_… almena tenant list   # …used without signing in
```

The sign-in is kept per profile in the system's keychain (macOS Keychain,
Windows Credential Manager, the Secret Service on Linux), or in
`credentials.toml` beside the configuration (owner-only) when there is no
keychain or `ALMENA_CREDENTIAL_STORE=file`. A sign-in ends 12 hours after it
starts, or after 30 minutes unused; an API token lasts the days it was made
for (up to 365). Logging out of an API token only forgets it: revoke it with
`almena token delete`.

Whatever the tenant signs — publishing an issuer, verifier or mediator, an
identity's DID log, a credential — is signed from the wallet of whoever signs
as the tenant: the command shows a QR code and waits for the wallet. Link
yours first with `almena account link-wallet`.

## Commands

Commands are `almena <resource> <verb>`. Lists are `list`, one item `get <id>`,
and `create`, `update <id>`, `delete <id>` (which asks first, unless `--yes`).

| Command | Description |
| --- | --- |
| `almena auth login \| logout \| status` | Sign in (email code, `--wallet`, `--with-token` from stdin), out, and who is signed in |
| `almena account get \| update \| ways-in` | Your account and its alias; its email and linked accounts |
| `almena account link-email \| unlink-email \| link-wallet \| unlink <id>` | Link or unlink ways in |
| `almena token list \| create \| delete` | API tokens for scripts and CI |
| `almena tenant list \| get \| update \| health` | Your tenants; the tenant's name, mediator and signing flow; what it still needs |
| `almena tenant use <id-or-name>` | Work in that tenant from now on (this profile) |
| `almena issuer list \| get \| create \| update \| delete` | The tenant's issuers (`verifier` the same, for verifiers) |
| `almena issuer publish \| unpublish <id>` | Publish (endorsed from the wallet), or back to a draft |
| `almena issuer signing get \| set <id>` | The member whose wallet signs for it |
| `almena verifier verify <id> --form <form-id>` | Ask a wallet, by QR in the terminal, to present what the form asks for as that verifier (published); waits and shows the verdict |
| `almena issuer queue get \| create \| rotate \| delete <id>` | Its queue at the broker, where its back office reads what happens to it (`verifier` the same); `create` and `rotate` print the user's password, once |
| `almena issuer credential-types get \| set <id>` | The credential types it grants, and the form of each offer |
| `almena issuer status-list list \| sign <id>` | Its status lists (entries used, revoked, suspended, whether to sign), and signing one from the wallet (`--status-list`; the current one by default) |
| `almena mediator list \| get \| create \| update \| delete \| publish \| unpublish` | The tenant's mediators |
| `almena mediator choices` | The mediators the tenant, its issuers and verifiers may pick |
| `almena identity list \| get \| create \| sign <id>` | The tenant's identities; sign an identity's next DID log entry |
| `almena signature list` | Identities whose DID waits for a signature |
| `almena domain list \| add <domain> \| check <id> \| remove <id>` | Linked domains, proved by a DNS TXT record |
| `almena member list \| invite` | Members and invitations |
| `almena form list \| get \| create` | The tenant's forms (from flags, or `--file` with the API's body) |
| `almena form schema \| dcql \| verify <id>` | Its answers' JSON Schema, its DCQL query; verify presented credentials |
| `almena field list \| create \| delete` | The tenant's own fields (`custom:{key}` in forms) |
| `almena catalog fields \| credentials \| issuers \| verifiers \| mediators \| offers` | Almena's public catalogues and what is published (no sign-in); `issuers --search <text> --grants <type>` narrows them |
| `almena catalog offer <issuer-slug> <type>` | An issuer's offer |
| `almena application list \| get \| file \| decide` | Applications the tenant's issuers received; accept or reject one |
| `almena issuance get \| set \| sign <application>` | Settle an accepted application's credential and sign it from the wallet |
| `almena application credential-status <id> valid\|suspended\|revoked` | Suspend, reinstate or revoke an issued credential: the issuer's signer signs its status list from the wallet; revoking asks first |
| `almena agent card \| ask \| chat` | Talk to the Almena agent (A2A); `ask` streams the answer |
| `almena config show \| set \| unset \| path` | The profile's settings |
| `almena completions <shell>` | Print the completion script for bash, zsh, fish, elvish or PowerShell |

`almena <command> --help` has every option.

## Global options

| Option | Environment | Default | |
| --- | --- | --- | --- |
| `--profile` | `ALMENA_PROFILE` | `default` | Configuration profile: its settings and its sign-in |
| `--api-url` | `ALMENA_API_URL` | `https://api.almena.id` | The Almena API |
| `--agent-url` | `ALMENA_AGENT_URL` | `https://agent.almena.id` | The Almena agent |
| `--tenant` | `ALMENA_TENANT` | the profile's, or your only one | Tenant to work in, by id or name |
| `-o, --output` | `ALMENA_OUTPUT` | `table` | `table`, or `json`: the API's answer as it came |
| `--locale` | `ALMENA_LOCALE` | `en` | `en` or `es`: emails, the wallet's sheet, texts shown |
| `-y, --yes` | | | Answer yes to every confirmation |
| `-v, --verbose` | `ALMENA_VERBOSE` | | Print each request |

Unset, an option takes the profile's value (`almena config set <key> <value>`,
kept in `config.toml`; `almena config path` says where), then its default.
`ALMENA_TOKEN` signs in with an API token; `ALMENA_CONFIG_DIR` moves the
configuration; `ALMENA_CREDENTIAL_STORE=file` keeps sign-ins out of the keychain.

Results go to stdout, notices and progress to stderr. Exit statuses: `0` done,
`1` failed, `2` wrong usage, `3` not signed in or not allowed, `4` not found,
`5` refused by the API (invalid, conflicting, expired). A refusal names the
API's code, e.g. `error: not_a_signer (403)`.

## License

Apache-2.0, see [LICENSE](LICENSE).
