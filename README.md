# AgentForge-RS

```
 █████╗  ██████╗███████╗███╗   ██╗████████╗███████╗ ██████╗ ██████╗  ██████╗███████╗
██╔══██╗██╔════╝██╔════╝████╗  ██║╚══██╔══╝██╔════╝██╔═══██╗██╔══██╗██╔════╝██╔════╝
███████║██║  ███╗█████╗  ██╔██╗ ██║   ██║   █████╗  ██║   ██║██████╔╝██║  ███╗█████╗
██╔══██║██║   ██║██╔══╝  ██║╚██╗██║   ██║   ██╔══╝  ██║   ██║██╔══██╗██║   ██║██╔══╝
██║  ██║╚██████╔╝███████╗██║ ╚████║   ██║   ██║     ╚██████╔╝██║  ██║╚██████╔╝███████╗
╚═╝  ╚═╝ ╚═════╝╚══════╝╚═╝  ╚═══╝   ╚═╝   ╚═╝      ╚═════╝ ╚═╝  ╚═╝ ╚═════╝╚══════╝
```

---

## ◆ PULSE

Your AI agent will behave how your rules tell it to. AgentForge-RS is the
`cargo` subcommand that stops the guessing: one command places a
battle-tested `AGENTS-RUST.md` constitution in the project root - 14
enforceable sections on safety, idiom, validation, and refactoring - with
a formal `[OVERRIDE §X]` system so real projects bend the rules without
breaking them. One command. Zero configuration. The rules arrive as a
typed, versioned, verifiable artifact - not a blob of prose.

| P0-P4 ▣ | P5 ☐ | P6 ◐ | P7-P11 ☐ |
|---|---|---|---|

*Foundation, the typed rule model, the manifest format, the conflict-safe
installer, the template engine (7 domain fragments), the golden-rule
safety gate, and the performance budgets are sealed. The rules update
pipeline (network half), the TUI, security hardening, and the v1.0 gate
stand open.*

> Built with pure Rust. The install path never touches the network -
> the constitution is embedded at compile time.
>
> **suradet-ps**, artifact keeper

---

## ◆ IGNITION

Install once, forge everywhere.

```
⟫ cargo install --git https://github.com/suradet-ps/agentforge-rs cargo-agentforge
⟫ cd your-rust-project
⟫ cargo agentforge init                      # install or upgrade the constitution
⟫ cargo agentforge init --template wasm,tauri  # compose domain fragments into it
⟫ cargo agentforge templates                 # list the available fragments
⟫ cargo agentforge check                     # up to date? exits non-zero when stale
⟫ cargo agentforge validate                  # every issue in the installed file, with line numbers
⟫ cargo agentforge diff                      # rule-level diff vs the target (honors local edits)
⟫ cargo agentforge verify [--template …]     # validation pipeline: is this ruleset shippable?
⟫ cargo agentforge version
```

`AGENTS-RUST.md` is now in your project root. Your AI agent reads it,
and follows it. `init` is the default subcommand, so bare
`⟫ cargo agentforge` behaves the same (core constitution only, written
verbatim).

Domain fragments (`wasm`, `tauri`, `bevy`, `embedded`, `axum`, `cli`,
`library`) extend the constitution with typed, namespaced rules that merge
cleanly into the core — no network, everything embedded at compile time.

`check`, `validate`, `diff`, and `verify` also accept `--json` for CI
consumption; every command exits `0` on success and a documented non-zero
code when something needs attention (conflict `3`, stale `4`, not-installed
`5`, diff `6`, input error `2`). Reproducible builds are pinned via
`SOURCE_DATE_EPOCH`, which fixes the manifest timestamp.

Update the rules: `⟫ cargo install --git ... --force` to pull the latest
baseline; `⟫ cargo agentforge init --force` to overwrite a locally edited
constitution; `⟫ cargo agentforge init --dry-run` to preview before
writing.

<details>
<summary>What the CLI does</summary>

| Situation | Action |
|---|---|
| No `AGENTS-RUST.md` | Embeds the latest baseline template, writes it to root |
| Already present | Skips the install, prints a reminder |
| Edited locally | Reports the diff; never overwrites without explicit `--force` |
| Stale baseline | `check` exits non-zero via the versioned manifest; upgrade is deliberate |

</details>

The template is written **verbatim** — tooling never rewrites the
constitution from the manifest, so the battle-tested prose survives
installs and upgrades.

---

## ◆ ANATOMY

Four crates, one contract: the rules are a typed model, not prose.

- **Models** - `agentforge-domain` turns the constitution into typed
  entities: `RuleId` (validated, orderable — numeric for the core,
  namespaced like `WASM-1` for fragments), `Rule` with severity and
  machine-readable tags, `Override` with target validation, the
  `RuleSet` that rejects duplicate ids and orphan overrides, and a
  code-fence-aware markdown parser that maps `AGENTS-RUST.md` into that
  model without ever panicking. 51 tests hold the model together.
- **Manifests** - every rule carries a body checksum in a versioned
  `.agentforge.json` companion, so tooling knows "what version of the
  rules is installed" without parsing prose - and can tell a pristine
  baseline from a locally edited rule.
- **Installs** - `agentforge-core` runs the flow: detect, parse, compare
  manifest, then `install` / `skip` / `upgrade` / `conflict`, plus
  `check` (non-zero when stale). Local edits are never overwritten
  silently - the diff is reported, `--force` is required, `--dry-run`
  touches nothing. Filesystem access stays behind a trait so the whole
  flow is unit-tested against an in-memory tree.
- **Builds** - `agentforge-builder` composes the core constitution with
  zero or more domain fragments (`wasm`, `tauri`, `bevy`, `embedded`,
  `axum`, `cli`, `library`), embedded at compile time, merged with
  collision/orphan-override validation and rendered deterministically.
  Every shipped template must merge and round-trip through the manifest
  before it ships.
- **Guards** - a golden-rule gate (`tests/golden_rules.json` +
  `check_golden_rules`) fails the build if the shipped ruleset ever drops
  or weakens a mandatory rule (e.g. §3 `clippy -D warnings`, §5.4
  `// SAFETY:`), backed by seeded property tests, a cargo-fuzz target
  asserting the parser never panics, and a validation pipeline
  (`verify`) that refuses to ship a ruleset with build errors.
  `SOURCE_DATE_EPOCH` pins manifests for byte-identical reproducible
  builds.
- **Ships** - the `cargo-agentforge` binary is the cargo subcommand
  itself: `init [--template ...]` / `check` / `version` / `templates` /
  `validate` / `diff` / `verify` over a dependency-light CLI, the
  pure-logic core kept dependency-free behind a filesystem trait,
  cross-platform release builds, checksums on every release, offline by
  construction. Perf budgets are enforced in CI (`scripts/perf-check.sh`):
  `<5MB` stripped binary, `<100ms` startup, `<50ms` core install,
  `<20MB` resident.

---

## ◆ RITUALS

**The core ceremony** - bootstrap any Rust project:

1. `cargo agentforge` in the project root.
2. The constitution appears: 14 sections from agent behavior through
   anti-patterns, ending at the Project Overrides section.
3. Where the project genuinely differs, write `[OVERRIDE §X]` lines -
   agents read them at runtime and adjust without breaking baseline
   compliance.
4. The mandatory checks take over: `clippy -D warnings`, `fmt`, `test`,
   `doc` - the gates your agents will hold.

**The ceremony of safety** - nothing is overwritten without consent. A
locally edited rule is reported, never silently replaced. `--dry-run`
prints exactly what would change and changes nothing.

**The ceremony of silence** - the install path is offline by default:
the template is embedded at compile time, no network, no phone-home.

---

## ◆ ECHOES

**Where this artifact is heading**

```
P0-P4 ▸ foundation, model, manifest, installer, template engine ─── ▸ sealed
P5    ▸ rules pipeline: reproducible builds + validation gate ▸ sealed; network updates ▸ open
P6    ▸ CLI surface: init/check/version/templates/validate/diff ▸ sealed
P7    ▸ TUI installer (ratatui), stretch ─────────────────────────── ▸ open
P8    ▸ golden-rule gate, property tests, fuzz target ────────────── ▸ sealed
P9    ▸ perf budgets: size <5MB, startup <100ms, install <50ms ▸ ▸ ▸ sealed
P10   ▸ security hardening ───────────────────────────────────────── ▸ open
P11   ▸ v1.0.0: crates.io publish, docs book, release gate ──────── ▸ open
```

**Raising the artifact** - this is a living standard. Bug reports,
AI edge-case discoveries, rule refinements, anti-pattern additions, and
domain templates are all welcomed - reference the `§` section you are
modifying. `CONTRIBUTING.md` and `SECURITY.md` hold the ground rules;
`ROADMAP.md` holds the honest path. The domain spec lives in
`docs/RULE_MANIFEST.md`.

**Status** - CI gates every change: fmt, `clippy -D warnings`, tests,
cargo-audit + cargo-deny, and cross-platform release builds
(Linux / Windows / macOS). [Watch the gates](.github/workflows).

---

```
  ─────────────────────────────────────────
   Your agent is only as disciplined
   as the rules you forge for it.
  ─────────────────────────────────────────
```

Distributed under the [MIT License](LICENSE).