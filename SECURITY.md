# Security Policy

## Supported Versions

The latest released version of `cargo-agentforge` receives security fixes.
Pre-1.0 versions are maintained on a best-effort basis.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in agentforge-rs, **do not open a
public issue.** Report it privately instead:

- Use GitHub's private vulnerability reporting (Security → Report a
  vulnerability) on the repository, or
- Email the maintainer directly (see the repository owner).

You can expect an acknowledgement within 72 hours. Once the issue is
confirmed, a fix and coordinated disclosure timeline will be agreed with
you. We will credit reporters who wish to be named.

## Scope Notes

- The CLI installs a local `AGENTS-RUST.md` file into the user's project.
  It performs no network access on the install path today (templates are
  embedded). Network access, if added later for ruleset updates, will
  always require explicit user confirmation and TLS validation that is
  never disabled.
- Supply-chain integrity for any future ruleset updates will be enforced
  via checksum/signature verification before applying.
