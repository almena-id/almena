# almena

Command-line client of the Almena ID platform. The binary is `almena`.

## Install

Every merge into `main` publishes a [release](https://github.com/almena-id/almena/releases)
with prebuilt binaries for Linux (x86_64, Ubuntu 24.04 or later), macOS (Apple
silicon) and Windows (x86_64), plus `SHA256SUMS`. Versions are
`year.month.sequence` (e.g. `2026.10.1`); `almena --version` reports it.

## Build

Requires a Rust toolchain (edition 2024) and [Task](https://taskfile.dev).

```sh
task build      # target/release/almena
task install    # into ~/.cargo/bin
task run -- --help
```

## Commands

| Command | Description |
| --- | --- |
| `almena completions <shell>` | Print the completion script for bash, zsh, fish, elvish or PowerShell |

Global options: `-v, --verbose` (or `ALMENA_VERBOSE`), `-h, --help`, `-V, --version`.

## License

Apache-2.0, see [LICENSE](LICENSE).
