# almena — notes for contributors and agents

`almena` is the command-line client of the Almena ID platform, written in Rust with `clap` (derive API).

## Layout

- `src/main.rs` — parses the command line, hands it to `commands::run`, and turns a failure into its exit status (3 sign-in or permission, 4 not found, 5 refused).
- `src/cli.rs` — the top of the command-line surface: `Cli`, `GlobalArgs` (options every subcommand accepts) and the `Command` enum; each resource's subcommands are declared in its own module.
- `src/context.rs` — `Context`: the global options settled against the profile, the sign-in (`ALMENA_TOKEN` first), API clients, the tenant to work in (`--tenant`, the profile's, or the only one) and confirmations.
- `src/config.rs` — profiles in `config.toml` (`ALMENA_CONFIG_DIR`, else the system's config directory).
- `src/credentials.rs` — each profile's sign-in, in the system keychain (`keyring`) or `credentials.toml` (owner-only).
- `src/client.rs` — the API client (`/api/v1`, blocking `reqwest`): JSON in and out, path segments escaped, refusals as `ApiError` with the API's code.
- `src/output.rs` — `--output table|json`: tables by column paths (`mediator.name`), texts by language in `--locale`; data to stdout, notices to stderr.
- `src/wallet.rs` — wallet requests: the QR code and deep link, polled until answered; the CLI asks as `client: "cli"`.
- `src/commands/` — one module per resource (`auth`, `account`, `token`, `tenant`, `issuer`, `verifier`, `mediator`, `identity`, `pending`, `domain`, `member`, `subscription`, `form`, `field`, `credential_type`, `category`, `value_domain`, `catalog`, `application`, `issuance`, `agent`, `config`, `completions`), each with its `Subcommand` enum and `run`; `described.rs` is what issuers, verifiers and mediators share; `mod.rs` dispatches and holds paging (`PageArgs`) and the `--file`/`LANG=TEXT` helpers.
- `tests/cli.rs` — the binary against a stand-in API (`wiremock`): what each command sends and prints.

- `.github/workflows/release.yml` — on every push to `main`: lint and test, build the binary for Linux, macOS (Developer ID, notarized) and Windows (Authenticode, Azure Trusted Signing), sign `SHA256SUMS` with the release OpenPGP key, and publish a GitHub release tagged `v<year>.<month>.<n>`. The version is compiled in through `ALMENA_VERSION` (`cli::VERSION`); without it, builds report Cargo.toml's version.
- `.github/workflows/distribute.yml` — run by hand: takes a published release (empty: the latest) and the channels to update. It moves the Homebrew formula in `almena-id/homebrew-tap` and the Scoop manifest in `almena-id/scoop-bucket` to it, each after installing it from a local copy of its repository, and opens a pull request for `AlmenaID.Almena` in `microsoft/winget-pkgs`. Checksums come from the release's `SHA256SUMS`; nothing is rebuilt.
- `packaging/` — the templates of those: `homebrew/almena.rb`, `scoop/almena.json` and the three manifests in `winget/`; the workflow fills in `@VERSION@` and the archives' checksums, reading them as they were at the release's tag.

## Adding a subcommand

1. A new resource: a variant (its doc comment is the help text) in `Command` in `src/cli.rs`, `src/commands/<resource>.rs` with its `Subcommand` enum and `run`, and a match arm in `src/commands/mod.rs`. A new verb: a variant in that resource's enum.
2. Name it `almena <resource> <verb>`: the resource singular, as the API names it; the verbs `list`, `get`, `create`, `update`, `delete`, or the API's own (`publish`, `sign`, `check`…). Flags are the API's fields in kebab-case (`--mediator` for `mediator_id`, `--no-<field>` to send `null`); ids are positional.
3. Print through `ctx.out` (`json` is the API's answer untouched); anything else goes to stderr with `output::notice`. Ask before what cannot be undone (`ctx.confirm`).
4. A test in `tests/cli.rs` that checks the request sent, and a row in the README command table.

## Rules

- Everything is written in English.
- Errors go through `anyhow`; no `unsafe`.
- Environment variables are prefixed `ALMENA_`.
- The API validates; the CLI checks only the shape of its arguments and never repeats the API's rules.
- Tokens never go on the command line: `ALMENA_TOKEN`, or `auth login --with-token` from stdin.
- Tasks live in `Taskfile.yml` (`task --list`). Before finishing a change: `task check` (fmt check, clippy -D warnings, tests).
