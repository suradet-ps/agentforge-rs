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

PR guidelines

- Keep PRs focused and small.
- Include tests for new behavior when possible.
- All CI checks should pass before merging.

License

By contributing you agree that your contributions will be licensed under the project's MIT License.
