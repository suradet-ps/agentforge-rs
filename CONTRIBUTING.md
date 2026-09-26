# Contributing

Thank you for considering contributing to agentforge-rs

How to contribute

1. Open an issue to discuss your change before starting.
2. Fork the repository and create a descriptive branch:
   git checkout -b feat/short-description
   or
   git checkout -b fix/short-description
3. Make changes and run formatting and checks:
   - cargo fmt --all
   - cargo clippy --all-targets --all-features -- -D warnings
   - cargo test
4. Write clear commit messages and reference the issue when applicable (e.g., "feat: add CONTRIBUTING.md (#123)").
5. Push your branch and open a Pull Request targeting the main branch. Include a short description and testing notes.
6. Be responsive to review comments and update your PR as requested.

Branch protection

`main` is protected: required status checks (fmt, clippy, test, cross-platform
build, audit, deny) must pass, force-push and deletion are disabled. PRs
require a passing CI run before merge.

PR guidelines

- Keep PRs focused and small.
- Include tests for new behavior when possible.
- All CI checks (`cargo fmt`, `cargo clippy -D warnings`, `cargo test`,
  cross-platform build, `cargo audit`, `cargo deny`) must pass before merging.

Ruleset releases

Ruleset distribution is independent of the CLI version, so rules can be
refreshed without reinstalling the binary.

1. Bump `RULESET_VERSION` in `crates/agentforge-builder/src/templates.rs` and
   merge the bump to `main` once CI is green.
2. Tag that commit `rules-v<version>` (matching `RULESET_VERSION`) and push
   the tag. The `Ruleset release` workflow re-runs the validation pipeline,
   writes `agentforge-rules-<version>.json` with `cargo agentforge bundle`,
   and publishes it with `SHA256SUMS.txt` on GitHub Releases.
3. Users update with
   `cargo agentforge update-rules --ruleset-version <version>`.

License

By contributing you agree that your contributions will be licensed under the project's MIT License.
