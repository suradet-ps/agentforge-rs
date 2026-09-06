# Fuzzing

`cargo-fuzz` harness for `agentforge-domain::parse_agents_md` and
`agentforge_domain::validate_agents_md`.

The invariant: **every input must error or parse cleanly — never panic.**

## Prerequisites

- A nightly Rust toolchain (`rustup toolchain install nightly`)
- `cargo install cargo-fuzz`

## Running

```sh
cargo +nightly fuzz run parse
```

## Notes

- This crate is deliberately **not** a workspace member, so `cargo build`,
  `cargo test --workspace`, and CI never touch it. It only builds under
  `cargo +nightly fuzz run parse`.
- A deterministic, dependency-free panic sweep over seeded random inputs
  also runs in `crates/agentforge-domain/tests/properties.rs` on stable.