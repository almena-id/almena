# almena — notes for contributors and agents

`almena` is the command-line client of the Almena ID platform, written in Rust with `clap` (derive API).

## Layout

- `src/main.rs` — parses the command line and hands it to `commands::run`.
- `src/cli.rs` — the whole command-line surface: `Cli`, `GlobalArgs` (options every subcommand accepts) and the `Command` enum.
- `src/commands/` — one module per subcommand, each exposing `run`; `mod.rs` dispatches.
  - `completions.rs` — shell completion scripts (`clap_complete`).

- `.github/workflows/release.yml` — on every push to `main`: lint and test, build the binary for Linux, macOS and Windows, and publish a GitHub release tagged `v<year>.<month>.<n>`. The version is compiled in through `ALMENA_VERSION` (`cli::VERSION`); without it, builds report Cargo.toml's version.

## Adding a subcommand

1. Add a variant (with its doc comment, which becomes the help text) to `Command` in `src/cli.rs`.
2. Add `src/commands/<name>.rs` with a `run` function and a match arm in `src/commands/mod.rs`.
3. Add it to the README command table.

## Rules

- Everything is written in English.
- Errors go through `anyhow`; no `unsafe`.
- Environment variables are prefixed `ALMENA_`.
- Tasks live in `Taskfile.yml` (`task --list`). Before finishing a change: `task check` (fmt check, clippy -D warnings, tests).
