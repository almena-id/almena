# Contributing

Thanks for helping with `almena`, the command-line client of Almena ID. By
taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
Security issues go through [SECURITY.md](SECURITY.md), never through public
issues.

## Getting started

You need a recent stable Rust and [Task](https://taskfile.dev).

```bash
task run -- --help  # runs the CLI from source
task build          # target/release/almena
task --list         # everything else
```

[README.md](README.md) lists the commands; [AGENTS.md](AGENTS.md) describes
the layout of the code and how to add a subcommand.

## Making a change

- Open an issue first for anything larger than a small fix, so the approach
  can be agreed before the code.
- Everything is written in English: code, comments, docs, help texts, commit
  messages.
- Errors go through `anyhow`; `unsafe` is forbidden. Environment variables are
  prefixed `ALMENA_`.
- Every subcommand is documented by its doc comment (the `--help` text) and
  listed in the README command table.
- New behaviour comes with tests.
- Before sending a change, `task check` must pass: formatting, clippy with
  warnings as errors, and all tests.

## Pull requests

Keep a pull request to one topic, describe what it changes and why, and link
the issue it addresses. By contributing you agree that your contribution is
licensed under the [Apache License 2.0](LICENSE), as the rest of the project.
