# AgentForge-RS

> The official `cargo` subcommand to bootstrap AI-ready Rust projects with `AGENTS-RUST.md`.  
> One command. Zero configuration. Production-grade AI rules delivered instantly.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.95%2B-orange)](https://www.rust-lang.org)
[![Cargo CLI](https://img.shields.io/badge/Cargo-Subcommand-blue)]()

## What is this?

`agentforge-rs` provides a lightweight CLI tool (`cargo agentforge`) that places a battle-tested `AGENTS-RUST.md` baseline into your project root. This file acts as a **constitution for AI coding agents**, enforcing:

- Strict safety & idiomatic Rust patterns
- Mandatory `clippy`, `fmt`, `test`, and `doc` validation
- Predictable, tiered refactoring workflows
- A formal `[OVERRIDE §X]` system for project-specific exceptions

Stop manually copying rules. Stop guessing how your AI should behave. Just run `cargo agentforge` and start building.

## Quick Start

### 1. Install the CLI
```bash
cargo install --git https://github.com/suradet-ps/agentforge-rs cargo-agentforge
```

### 2. Use in Any Rust Project
```bash
cd your-rust-project
cargo agentforge
```

✅ `AGENTS-RUST.md` is now in your project root. Your AI agent will automatically read and follow it.

## How It Works

| Step | Action |
|------|--------|
| `cargo agentforge` | Checks if `AGENTS-RUST.md` already exists |
| Missing | Embeds the latest baseline template & writes it to root |
| Exists | Skips installation & prints a reminder |
| Update | Re-run `cargo install --git ... --force` to pull latest rules |

Built with pure Rust. Zero dependencies. Compiled once, works everywhere.

## What's Inside `AGENTS-RUST.md`?

The installed file contains **14 enforceable sections** covering:
- `§0–2` Agent behavior, interaction protocols & golden rules
- `§3–4` Mandatory checks, `Cargo.toml` & workspace standards
- `§5–7` Ownership, error handling, async/tokio patterns
- `§8–9` Testing hygiene & documentation standards
- `§10–11` Security checklist & CI/release workflows
- `§12–13` Refactoring workflow & anti-pattern table
- `§14` Project Overrides system for safe rule exceptions

[View the full `AGENTS-RUST.md` source](./templates/AGENTS-RUST.md)

## The Override System (Built-In)

Real projects need flexibility. Add exceptions directly in the generated file:
```markdown
## 14. Project Overrides
[OVERRIDE §5.2] Use `anyhow` instead of `thiserror` for faster iteration
[OVERRIDE §4.1] edition = "2021" (legacy dependency requirement)
```
AI agents read overrides at runtime and adjust behavior without breaking baseline compliance.

## Maintenance & Updates

```bash
# Update the CLI to the latest version
cargo install --git https://github.com/suradet-ps/agentforge-rs cargo-agentforge --force

# Update the rules in an existing project
rm AGENTS-RUST.md && cargo agentforge
```

## Contributing

This project is a living standard. We welcome:
- Bug reports & AI edge-case discoveries
- Rule refinements, anti-pattern additions, or workflow improvements
- Translations or domain-specific templates (Tauri, Bevy, Embedded, WASM)

Open an issue or PR. Please reference the `§` section you're modifying.

## License

Distributed under the **MIT License**. See `LICENSE` for details.
