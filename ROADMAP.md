# AgentForge-RS Roadmap

This roadmap tracks the path from the current scaffold to a first public
release, and beyond. It follows the architecture and requirements set out in
[README.md](README.md) and [CONTRIBUTING.md](CONTRIBUTING.md). Nothing here
ships until the "zero broken install" policy and the golden rule-suite both
pass in CI.

The north star: `agentforge-rs` should become the **single standard way to
bootstrap AI-coding-agent rules in the Rust ecosystem** — a `cargo`
subcommand that installs, validates, version-checks, and updates a
battle-tested `AGENTS-RUST.md` constitution, with a growing library of
domain-specific templates (WASM, Tauri, Bevy, Embedded, axum, …), a
machine-readable rules manifest, and a reproducible offline-install story.

## Phase 0: Foundation (done)

- [x] Cargo package `cargo-agentforge`, `cargo` subcommand convention (`cargo-agentforge` binary, invoked as `cargo agentforge`)
- [x] `src/main.rs` minimal CLI: detects existing `AGENTS-RUST.md`, installs baseline template, skips when present
- [x] `templates/AGENTS-RUST.md`: 14-section constitution with `[OVERRIDE §X]` system
- [x] MIT license, `CONTRIBUTING.md`, `rustfmt.toml`
- [x] README with quick start, override-system docs, maintenance flow
- [x] GitHub issue/PR templates (under `.github/`): PR, bug report, rule refinement, security report
- [x] CI: fmt, clippy `-D warnings`, test, cross-platform release build (x86_64-unknown-linux-gnu / x86_64-pc-windows-msvc / x86_64-apple-darwin)
- [x] cargo-audit + cargo-deny policy set (`deny.toml`), enforced in CI; currently zero deps, the gate stays as the tree grows
- [x] Release workflow: tag-triggered, checksums (`SHA256SUMS.txt`), GitHub release via `gh` CLI
- [x] SECURITY.md (private reporting, no-network install-path guarantee) and CODE_OF_CONDUCT.md (Contributor Covenant 2.1)
- [ ] Branch protection on `main`: required status checks (strict), no force-push, no deletion — **must be enabled in the GitHub repository settings** (not settable from a file). Suggested command captured in the Phase 0 PR body. CI already enforces the same checks on every PR.

## Phase 1: Domain / Rule Model (`agentforge-domain`) (done)

Today the rule set is a single opaque markdown blob. To version, validate,
and diff rules we need a structured, typed model of what a rule actually is.
This is the phase that makes everything downstream possible.

- [x] New crate `agentforge-domain` with zero `std::unwrap()` outside tests
- [x] `RuleId` newtype (e.g. `RuleId("5.2")`), never a bare `String` key floating through the code; validated against `[0-9.]` pattern, numeric ordering (`5` < `5.2` < `14`); extended to namespaced ids (`WASM-1`, `WASM-1.2`) for domain fragments, numeric core ids sort before namespaced ids
- [x] `Rule` entity: id, section, title, body, severity (`Mandatory` / `Recommended` / `Advisory`), machine-readable tag set (`tokio`, `unsafe`, `testing`, …); case-insensitive tag lookup
- [x] `Override` entity: a `[OVERRIDE §X]` parsed into target rule id + replacement / exemption, with validation that the target id actually exists in the baseline; `parse_line()` handles the bracket/section format
- [x] `RuleSet` (the parsed `AGENTS-RUST.md`): ordered rules + overrides + recorded section headings (`Section { id, title }`), with `add_rule()` / `add_override()` / `add_section()` validation (duplicate IDs rejected, override targets must exist), lookup by ID
- [x] Markdown → `RuleSet` parser (`parse_agents_md`): code-fence-aware extraction of sections (`## N` or `## WASM-1`), sub-rules (`### N.N`), and `[OVERRIDE §X]` directives; sections without sub-rules become single rules; typed errors, never panics; the bundled 14-section template parses into 27 rules
- [x] Lenient validator (`validate_agents_md`): same scan reports **every** issue with line numbers (`MalformedHeading`, `DuplicateRuleId`, `DuplicateSection`, `DuplicateOverride`, `MalformedOverride`, `OrphanOverride`, `EmptyRuleSet`) instead of stopping at the first; JSON-serializable `ValidationReport`
- [x] `RuleManifest` (new, see Phase 2): versioned, machine-readable companion to the markdown, so tooling never has to parse prose to know "what version of the rules is installed"; `from_rule_set()` with per-rule body checksums (FNV-1a placeholder for SHA-256), JSON serializable
- [x] Domain error types with `thiserror`; typed errors for `InvalidRuleId`, `DuplicateRuleId`, `OverrideTargetNotFound`, `DuplicateOverride`, `EmptyRuleSet`, `MissingField`, `ManifestVersionMismatch`
- [x] Unit tests: 30 tests covering rule-id ordering, override-target validation, duplicate-rule rejection, severity ordering, override parsing (valid/invalid/edge cases), manifest generation, serialization round-trip
- [x] Workspace restructured: root `Cargo.toml` now a workspace with `crates/cargo-agentforge` and `crates/agentforge-domain`
- [x] `serde` + `thiserror` added to workspace dependencies; `serde_json` as dev-dependency for manifest tests

## Phase 2: Rule Manifest Format (`.agentforge.json` / versioned) (done)

The markdown is for humans and agents. Tooling (CI, the CLI's own
version-check, future IDE plugins) needs a stable, parseable artifact.

- [x] Manifest fields: `manifest_version`, `ruleset_version`, `generated_at`, `rule_count`, `rules[]` (id, section, severity, tags, checksum of body), `overrides[]` — defined as `RuleManifest` / `ManifestRule` / `ManifestOverride` in `agentforge-domain`
- [x] Per-rule body checksum so the CLI can detect "the user edited this rule locally" vs "this is a pristine baseline rule" — the foundation of safe updates (Phase 6); currently FNV-1a, will swap for real SHA-256
- [x] Schema validation via `serde` deserialization; rejects malformed manifests with typed errors, never panics
- [x] Format specification written down (`docs/RULE_MANIFEST.md`): schema, versioning strategy (semver of the rule set, independent of the CLI version), forward/backward compatibility rules
- [x] Round-trip test: parse baseline → emit manifest → re-read manifest → identical effective rule set

## Phase 3: Installer Rewrite (`agentforge-core`) (done)

Replace the current "copy the blob if missing" logic with a typed,
idempotent, auditable installer built on the domain model.

- [x] Crate `agentforge-core` depends only on `agentforge-domain`; no filesystem access in the pure-logic layer (FS isolated behind a trait so it is unit-testable)
- [x] `InstallTarget` trait (filesystem abstraction) so install/upgrade/dry-run can be tested against an in-memory tree
- [x] Install flow: detect existing file → parse → compare manifest → decide `install` / `skip` / `upgrade` / `conflict`; the `AGENTS-RUST.md` template is written **verbatim** (never regenerated from the manifest), and skip-detection ignores `generated_at` so re-installs of an identical ruleset are idempotent
- [x] Never overwrite a locally-edited rule silently: if a rule's body checksum differs from the baseline, the CLI reports the diff and requires explicit `--force` (the "zero silent data loss" policy)
- [x] `--dry-run` that prints exactly what would change, no writes
- [x] `--check` that exits non-zero (with a report) if the installed ruleset is older than the bundled baseline (`check_status` in `agentforge-core`; `CheckStatus::{UpToDate, Stale, NotInstalled}`, `ExitCode::Stale`/`NotInstalled`)
- [x] Exit codes distinguishing: `installed`, `upgraded`, `skipped`, `conflict-needs-confirmation`, `input-error`, `internal-error`
- [x] Unit tests: install-when-missing, skip-when-pristine, conflict-when-edited, force-overwrite, dry-run-touches-nothing, version-mismatch, corrupt-manifest

## Phase 4: Template Engine & Domain Templates (`agentforge-builder`) (done)

One monolithic `AGENTS-RUST.md` cannot serve WASM, kernel, and web
equally. The constitution becomes a **core + pluggable domain layer**.

- [x] Crate `agentforge-builder`: composes the core constitution with zero or more domain template fragments into the final `AGENTS-RUST.md`; core-only builds return the verbatim template, composed builds are re-rendered deterministically
- [x] Fragment format: each domain template is itself a validated `RuleSet` fragment with its own namespaced section range (`§WASM-1..`, `§TAURI-1..`), merged without id collisions
- [x] Domain templates shipped in-repo: `wasm`, `tauri`, `bevy`, `embedded-no-std`, `axum`, `cli`, `library` (the README already promises Tauri/Bevy/Embedded/WASM); each embedded at compile time in `agentforge-builder` alongside the core constitution
- [x] `cargo agentforge init --template wasm,tauri` selects fragments; default = core only (bare `init` keeps writing the verbatim core template; unknown templates error with `ExitCode::InputError`)
- [x] Merge validation: no rule-id collision across fragments, no orphan overrides, section ordering stable and deterministic; fragments may target core rules with overrides (validated against the composed set)
- [x] Offline by default: all templates embedded at compile time (like today's single template), no network on the install path
- [x] Unit tests: each promised domain template compiles, merges cleanly, round-trips through the manifest (`every_shipped_template_merges_cleanly`, `every_shipped_template_round_trips_through_manifest`)

## Phase 5: Data / Rules Pipeline & Distribution (offline half done)

The README's "update the rules" story today is `cargo install --force`. That
pulls the whole CLI binary just to refresh a text file. We separate **rule
distribution** from **CLI distribution**.

- [ ] `cargo agentforge update-rules` fetches the latest ruleset manifest + markdown from a pinned, TLS-validated URL (GitHub Releases asset), never disabling cert validation
- [ ] Ruleset published as a standalone release asset (`agentforge-rules-<version>.tar.zst`) separate from the binary, so updating rules does not require reinstalling the CLI
- [ ] Reproducible build: same input manifest → byte-identical `AGENTS-RUST.md` output (deterministic section ordering, no timestamps in output unless `--emit-metadata`)
- [x] `SOURCE_DATE_EPOCH` support for reproducible builds (`generated_at_from_epoch` — hand-rolled civil-from-days, no `chrono` dep)
- [x] Validation pipeline: a ruleset build with override-target errors or rule-id collisions must not produce a shippable manifest (`validation_report` in `agentforge-builder`)
- [x] `validation-report.json` output (`errors`, `warnings`, `rule_count`, `fragment_count`); warns when an override weakens a `Mandatory` rule; surfaced as `cargo agentforge verify [--template …] [--json]`
- [ ] Unit + integration tests against a fixture ruleset; the live-network fetch is `#[ignore]`d like MenSung's real-API tests

## Phase 6: CLI Surface (`cargo-agentforge`) (open)

Expand the single install command into a coherent, scriptable CLI.

- [x] `cargo agentforge init [--force] [--dry-run]` — install/upgrade, wired to `agentforge-core` (default subcommand; `--template` lands with Phase 4)
- [x] `cargo agentforge check` — report installed ruleset version vs bundled baseline, non-zero on stale (`ExitCode::Stale`/`NotInstalled`); "vs latest" (network) lands with Phase 5
- [ ] `cargo agentforge update-rules [--yes]` — explicit, confirmed network fetch; never automatic (Phase 5)
- [x] `cargo agentforge diff` — rule-level diff between installed and target ruleset, honoring local edits (parses the actual `AGENTS-RUST.md`, detects the installed template selection, compares body checksums; `ExitCode::HasDiff` on any change)
- [x] `cargo agentforge validate` — parses the project's `AGENTS-RUST.md` and reports **every** malformed override, orphan override, duplicate id, and malformed heading with line numbers (`validate_agents_md`; non-zero on issues)
- [x] `cargo agentforge version` — prints CLI version, never touches network or filesystem
- [x] `cargo agentforge templates` — lists available domain templates and their descriptions
- [x] Plain-text and `--json` output for `check`/`validate`/`diff` so CI can consume them
- [ ] All network-touching commands gated on explicit user confirmation; no silent phone-home

## Phase 7: TUI / Interactive Installer (open, stretch)

- [ ] `ratatui` + `crossterm` interactive `cargo agentforge tui`
- [ ] Template picker with checkboxes, shows what each fragment adds
- [ ] Live preview pane rendering the composed `AGENTS-RUST.md` as the user toggles fragments
- [ ] Explicit confirm step before any write; Esc/Ctrl-C abort with zero changes
- [ ] Keyboard-only navigation (no mouse required)
- [ ] Verified interactively in a real terminal (tmux), with a regression test for "cancel clears selection, does not write"

## Phase 8: Safety / Golden Rule Suite (done)

The equivalent of MenSung's medical safety gate: we must never ship a
ruleset that is internally inconsistent or that silently weakens a
mandatory rule.

- [x] `tests/golden_rules.json`: fixed known cases — e.g. "§5 mandates `clippy -D warnings`" must survive a build; "an override targeting a nonexistent rule is rejected"; "two fragments must not collide on a rule id"
- [x] CI gate: `crates/agentforge-builder/tests/golden_rules.rs` runs under `cargo test --workspace`; a ruleset that drops or weakens a golden rule fails it (`check_golden_rules` in `agentforge-builder`)
- [x] `cargo-fuzz` target for the markdown→RuleSet parser (`fuzz/`, runs via `cargo +nightly fuzz run parse`, deliberately outside the workspace so stable builds/CI never compile it; every malformed input must error, never panic)
- [x] Property-based tests for override resolution (deterministic seeded-PRNG sweeps: every override resolves to an existing rule, orphans are rejected, applying then reverting is a no-op on pristine rules; parse→render→parse round-trip is stable and rendering is idempotent across 500 generated documents; the parser never panics on 3000 random malformed inputs)
- [x] "Zero silent downgrade" test: removing or weakening a `Mandatory` golden rule (e.g. §3, §5.2) is detected by the gate

## Phase 9: Performance Hardening (open)

- [ ] Benchmark harness (criterion) for cold-start and install time
- [ ] Install `<50ms` for core-only on a warm filesystem, `<200ms` with all templates, verified in CI not just claimed
- [ ] Binary size budget check as a CI step (target: `<5MB` stripped, zero-dep core); fails the build if exceeded
- [ ] Memory budget (`<20MB` resident) on a constrained runner
- [ ] Startup `<100ms` measured from `cargo agentforge version` (the no-op path)

## Phase 10: Security Hardening (open)

- [ ] Full `unsafe` audit: currently zero `unsafe`; keep it that way and document the invariant in CI
- [ ] cargo-audit + cargo-deny stay green as dependencies (serde, ratatui, …) are added
- [ ] Reproducible build verification: same input → byte-identical `AGENTS-RUST.md` and manifest
- [ ] Supply-chain: pin and checksum the ruleset release asset; verify signature/SHA256 before applying an update
- [ ] Never disable TLS validation on the update path; reject self-signed/mismatched certs with a typed error

## Phase 11: First Public Release & v1.0.0 (open)

- [ ] Full core constitution + the promised domain templates (WASM/Tauri/Bevy/Embedded) shipped and validated with zero errors
- [ ] `validation-report.json` with zero errors on the shipped ruleset
- [ ] Field/deploy guide: how to install offline (copy the bundled ruleset via USB/sneakernet), how to verify the checksum, how to report a rule bug
- [ ] `v1.0.0` tag, `release.yml` run, binaries + `SHA256SUMS.txt` published for Linux/Windows/macOS
- [ ] `crates.io` publish of `cargo-agentforge` with a stable `cargo install cargo-agentforge` path (today it is `--git` only)
- [ ] Docs site / book: the constitution explained section-by-section, with rationale for every `Mandatory` rule

## Future / Ecosystem

- [ ] Editor/IDE integrations (VS Code, Zed, Neovim) that read `AGENTS-RUST.md` and surface active rules inline
- [ ] `agentforge lint`: a standalone linter that checks a Rust project *actually conforms* to its installed ruleset (e.g. detects `#[allow(clippy::…)]` where the ruleset forbids it)
- [ ] Periodic offline ruleset updates distributable by USB for air-gapped CI runners
- [ ] ARM builds (Raspberry Pi / Apple Silicon native, CI already covers apple-darwin; add linux-arm64)
- [ ] Additional domain templates contributed by the community (actix, dioxus, leptos, no_std firmware, …) behind the same validation gate
- [ ] Translations / localized constitutions for non-English teams, each its own versioned fragment
- [ ] A public registry of community rule fragments, opt-in, with the same "no silent override, no panic on parse" guarantees
- [ ] `agentforge init --from <repo>`: bootstrap a project's *own* agent rules from a team template, versioned and diffable like the core set
