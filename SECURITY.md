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

- The workspace forbids `unsafe` code at the lint level
  (`[workspace.lints.rust] unsafe_code = "forbid"`). Every crate opts in via
  `[lints] workspace = true`, and the `unsafe-audit` CI job verifies the
  policy stays in place. There is no way for a single crate to opt out.
- The CLI installs a local `AGENTS-RUST.md` file into the user's project.
  The install path performs no network access (templates are embedded).
- `cargo agentforge update-rules` is the only network command. It asks for
  explicit confirmation before the first request (non-interactive runs must
  pass `--yes`), uses rustls with Mozilla's roots and never disables TLS
  validation, verifies the SHA-256 of the downloaded bundle, and refuses to
  overwrite locally-edited rules without `--force`.
- Supply-chain integrity for ruleset updates is enforced via SHA-256
  verification before applying; signatures are planned (Phase 10).
