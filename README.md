# almena

Command-line client of the Almena ID platform. The binary is `almena`.

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
