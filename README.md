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

| P0 ▣ | P1 ▣ | P2 ▣ | P3 ▣ | P4-P11 ☐ |
|---|---|---|---|---|

*Foundation, the typed rule model, the manifest format, and the
conflict-safe installer are sealed. The template engine, update pipeline,
full CLI surface, and the v1.0 gate stand open.*

> Built with pure Rust - zero dependencies, compiled once, works
> everywhere. The install path never touches the network.
>
> **suradet-ps**, artifact keeper

---

## ◆ IGNITION

Install once, forge everywhere.

```
⟫ cargo install --git https://github.com/suradet-ps/agentforge-rs cargo-agentforge
⟫ cd your-rust-project
⟫ cargo agentforge
```

`AGENTS-RUST.md` is now in your project root. Your AI agent reads it,
and follows it.

Update the rules: `⟫ cargo install --git ... --force` to pull the latest
baseline; `⟫ rm AGENTS-RUST.md && cargo agentforge` to re-install.

<details>
<summary>What the CLI does</summary>

| Situation | Action |
|---|---|
| No `AGENTS-RUST.md` | Embeds the latest baseline template, writes it to root |
| Already present | Skips the install, prints a reminder |
| Edited locally | Reports the diff; never overwrites without explicit `--force` |
| Stale baseline | Detected via the versioned manifest; upgrade is deliberate |

</details>

---

## ◆ ANATOMY

Three crates, one contract: the rules are a typed model, not prose.

- **Models** - `agentforge-domain` turns the constitution into typed
  entities: `RuleId` (validated, orderable), `Rule` with severity and
  machine-readable tags, `Override` with target validation, and the
  `RuleSet` that rejects duplicate ids and orphan overrides. Thirty
  tests hold the model together.
- **Manifests** - every rule carries a body checksum in a versioned
  `.agentforge.json` companion, so tooling knows "what version of the
  rules is installed" without parsing prose - and can tell a pristine
  baseline from a locally edited rule.
- **Installs** - `agentforge-core` runs the flow: detect, parse, compare
  manifest, then `install` / `skip` / `upgrade` / `conflict`. Local edits
  are never overwritten silently - the diff is reported, `--force` is
  required, `--dry-run` touches nothing. Filesystem access stays behind a
  trait so the whole flow is unit-tested against an in-memory tree.
- **Ships** - the `cargo-agentforge` binary is the cargo subcommand
  itself: zero dependencies, cross-platform release builds, checksums on
  every release, offline by construction.

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
P0-P3 ▸ foundation, rule model, manifest, installer ────────────── ▸ sealed
P4    ▸ template engine: wasm, tauri, bevy, embedded, axum ──────── ▸ open
P5    ▸ rules pipeline: pinned TLS-fetched updates, reproducible ▸ ▸ ▸ open
P6    ▸ full CLI surface: init, check, diff, validate, templates ── ▸ open
P7    ▸ TUI installer (ratatui), stretch ─────────────────────────── ▸ open
P8-P10 ▸ golden-rule suite, perf budgets, security hardening ─────── ▸ open
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