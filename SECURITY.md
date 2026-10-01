# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub: on
[almena-id/almena](https://github.com/almena-id/almena), open the
**Security** tab and choose **Report a vulnerability**. Do not open a public
issue, pull request or discussion about it.

Include what you can of:

- the version (`almena --version`) or commit;
- what an attacker can do, and under which configuration;
- steps or a proof of concept to reproduce it.

We aim to acknowledge a report within 3 working days and to agree on a
disclosure date with you once the issue is understood. We credit reporters in
the release notes unless you prefer otherwise.

## Supported versions

The project is before its first release: only the `main` branch receives
security fixes.

## Scope

In scope, among others:

- anything that makes `almena` leak keys, tokens or other secrets it handles
  (to the terminal, logs, files or the network);
- unsafe handling of files, paths or input given to the CLI;
- issues in how the CLI talks to Almena ID services.

Out of scope:

- issues in the services themselves, which belong to their own repositories;
- vulnerabilities in third-party dependencies that `almena` does not expose.
