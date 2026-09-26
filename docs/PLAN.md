# Continuation Plan

Working plan from the current state to a complete v1.0.0 and beyond.
[ROADMAP.md](../ROADMAP.md) remains the source of truth for phases; this file
adds task-level detail, decisions, and resume instructions so work can pause
and restart cleanly.

Status snapshot: 2026-09-26.

## Where we are

Done phases (see ROADMAP for the full item lists):

- Phase 0 Foundation: done, branch protection enabled (11 required checks).
- Phase 1 Domain model, Phase 2 manifest, Phase 3 installer core, Phase 4
  templates, Phase 8 golden rules, Phase 9 performance: done.
- Phase 5 rules distribution: done. `cargo agentforge update-rules` fetches a
  JSON bundle, verifies SHA-256, and applies it; the `Ruleset release`
  workflow publishes bundles on `rules-v*` tags. Tag `rules-v0.1.0` was
  published and verified end to end (bundle 40 KB, checksums match, live
  `#[ignore]` test passed).
- Phase 6 CLI: complete except `check --remote` ("vs latest") and `update`
  currently requires an explicit `--ruleset-version` or `--url`; there is no
  "latest" resolution yet.
- Phase 10 Security: partially done. `unsafe_code = "forbid"` workspace lint
  with the `unsafe-audit` CI job, cargo-audit/cargo-deny green, reproducible
  build verification. Missing: ruleset signatures, a self-signed TLS
  rejection test.
- Phase 7 TUI and Phase 11 v1.0.0: open.

Merged recently: #26 unsafe audit, #27 Phase 0 tick, #28 reproducible build,
#29 real SHA-256, #31 core update logic, #32 update-rules CLI + JSON bundle,
#33 ruleset release workflow + `--ruleset-version`, #34 fetch retry with cache
buster.

Current test suite: 184 tests, 1 ignored (live network). Current CLI commands:
`init`, `check`, `diff`, `validate`, `verify`, `templates`, `version`,
`update-rules`, `bundle`.

## How we work

- One branch and one PR per task. The maintainer merges; do not push to
  `main` (branch protection requires all 11 checks).
- Local verification triplet before every PR:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`
- Live network test (only after a `rules-v*` release exists):
  `cargo test -p cargo-agentforge live_fetch_of_the_published_bundle -- --ignored`
- Offline update smoke test without touching the internet: run
  `cargo agentforge bundle --output <dir>/agentforge-rules-<v>.json`, adjust
  the version fields to simulate an upgrade, serve the directory with
  `python -m http.server`, and point `update-rules --url` at it.
- Ruleset release process (also in [CONTRIBUTING.md](../CONTRIBUTING.md)):
  bump `RULESET_VERSION` in `crates/agentforge-builder/src/templates.rs`,
  merge, then tag `rules-v<version>` and push the tag. The tag must match
  `RULESET_VERSION`.

## Remaining tasks

Tasks are ordered by dependency and value. Each is expected to be one PR.

### T1 (Phase 10) ruleset signatures [needs maintainer key material]

Goal: verify a signature before applying an update, on top of SHA-256.

Blocked on maintainer action:

1. Generate a minisign keypair (`minisign -G`).
2. Store the secret key (and password, if any) as GitHub Actions secrets,
   suggested names `MINISIGN_SECRET_KEY` and `MINISIGN_PASSWORD`.
3. Provide the public key for pinning in the repository.

Implementation outline once unblocked:

- Add a small verify crate (`minisign-verify`) to `cargo-agentforge` only,
  and update `deny.toml` if the license list needs it.
- Pin the public key in the CLI (compile-time constant or a checked-in
  `docs/` key file via `include_str!`).
- Extend `update-rules`: after the SHA-256 check, fetch the `.minisig` asset
  next to the bundle and verify it; a missing or invalid signature is a hard
  error (decide whether `--no-signature` is acceptable as an escape hatch,
  recommended: no, or default-on with an explicit opt-out documented in
  SECURITY.md).
- Extend `.github/workflows/ruleset-release.yml` to sign the bundle with
  minisign after checksumming and publish `agentforge-rules-<v>.json.minisig`.
- Tests: fixture signature verifies, tampered bundle fails, wrong key fails.
- Verification: publish `rules-v0.1.1` (bump `RULESET_VERSION` first) and run
  a real `update-rules --ruleset-version 0.1.1`.
- Tick the Phase 10 supply-chain checkbox when done.

### T2 (Phase 10) TLS rejection test and remaining ticks

- Add a test that a self-signed certificate is rejected: dev-dependencies
  `rcgen` (generate a certificate in the test) and `rustls` (run a local TLS
  listener), then assert `UreqFetcher::fetch` fails and the error is
  classified (consider adding a `RemoteError` TLS variant and mapping
  `ureq::Error::Tls` in `get_once`).
- Tick the "cargo-audit + cargo-deny stay green" item: CI already enforces
  both on every PR; no code change required beyond the checkbox.
- Note in SECURITY.md that Mozilla roots come from `webpki-roots`, so trust
  anchor updates ship with CLI releases.

### T3 (Phase 11) crates.io readiness

Goal: `cargo install cargo-agentforge` works without `--git`.

- Add `version = "0.1.0"` to the path dependencies in
  `crates/agentforge-core`, `crates/agentforge-builder`, and
  `crates/cargo-agentforge` so each crate is publishable in order
  (domain, core, builder, CLI).
- Add repository/readme/keywords/categories metadata (workspace package
  metadata where possible).
- Verify with `cargo package --list` and `cargo publish --dry-run` for each
  crate in dependency order (dry-run needs each earlier crate to exist on
  crates.io only for real publishing; use `--no-verify` if needed for the
  first local pass).
- Decide whether publishing is manual (documented in CONTRIBUTING) or a
  `workflow_dispatch` job with `CARGO_REGISTRY_TOKEN`.
- Update README install instructions once the first publish lands.

### T4 (Phase 11) validation-report.json on the shipped ruleset

- Make `cargo agentforge verify --json` output the validation report for the
  full template selection, and have the `Ruleset release` workflow upload it
  as an artifact and/or release asset named `validation-report.json`.
- Add a test or workflow assertion that the shipped report has zero errors
  (the workflow already fails on a non-zero exit, this makes the artifact
  explicit).
- Tick the Phase 11 item for zero-error validation reports.

### T5 (Phase 11) deploy and field guide

Create `docs/DEPLOY.md` covering:

- normal install (`cargo install cargo-agentforge`, init, update-rules);
- offline and air-gapped install: copy the bundle JSON and `SHA256SUMS.txt`
  via USB, verify with `sha256sum`, apply with `update-rules --url`;
- hosting your own bundle for a team (run `bundle` in CI, publish to S3 or
  an internal artifact store, point everyone at `--url`);
- verification steps (checksum, `validate`, `diff`) and how to report a bad
  rule;
- version policy: CLI version vs ruleset version, `rules-v*` tags.
- Link it from README and CONTRIBUTING.

### T6 (Phase 11) v1.0.0 release

- Decide whether `RULESET_VERSION` also moves to 1.0.0 (it is independent;
  recommended: keep the ruleset at its own version unless rules changed).
- Bump crate versions to 1.0.0 (four manifests plus path-dependency version
  fields plus `Cargo.lock`).
- Run the full-template validation gate and commit or attach the report.
- Tag `v1.0.0`: `release.yml` publishes binaries and `SHA256SUMS.txt` for
  Linux/Windows/macOS.
- Publish crates to crates.io in order.
- Post-release: verify `cargo install cargo-agentforge`, verify `--ruleset-version`
  still works from a clean machine, update README.

### T7 (Phase 5/6 follow-up) `check --remote` and latest resolution

- Add `check --remote (--ruleset-version <v> | --url <bundle>)`: fetch and
  verify the remote bundle, compare versions, exit `Stale` when the installed
  ruleset is older, without writing.
- Optionally resolve "latest" via the GitHub releases API filtered to
  `rules-v*` tags (decide on rate-limit handling and whether this stays
  opt-in). This closes the "vs latest" note on the `check` ROADMAP item.

### T8 (Phase 7, stretch) interactive TUI

- `ratatui` + `crossterm`: `cargo agentforge tui`.
- Template picker with checkboxes, live preview of the composed markdown,
  explicit confirm before writing, keyboard-only navigation, Esc/Ctrl-C
  aborts with zero writes.
- Regression test: cancel clears the selection and writes nothing.
- Watch the perf job: new dependencies count against the 5 MB stripped
  binary budget.

### T9 (Future, as adoption dictates)

- `linux-arm64` release target in the build matrix.
- `agentforge lint` (check a project actually conforms to its ruleset).
- IDE integrations (VS Code, Zed, Neovim) reading `.agentforge.json`.
- Community template gate, translations, and `init --from <repo>`.
- Periodic offline ruleset bundles for air-gapped CI (overlaps T5).

## Outstanding decisions

1. Minisign key material for T1 (maintainer action) and whether signatures
   become mandatory by default.
2. Whether ruleset version and CLI version align at 1.0.0 (T6).
3. crates.io publishing: manual versus token-authenticated workflow (T3).
4. "Latest ruleset" resolution for T7: pin explicitly, or query the GitHub
   releases API.

## Invariants and gotchas

- `check` compares only the `ruleset_version` string in `.agentforge.json`
  against the bundled constant; it does not read the markdown and ignores
  templates. Use `validate` and `diff` as real gates until T7 lands.
- GitHub's release download edge once pinned a persistent HTTP 500 to the
  exact `SHA256SUMS.txt` URL (any query string worked). `UreqFetcher` now
  retries with a `url_retry` cache-buster; do not remove it.
- `update-rules` in non-interactive sessions requires `--yes`; `--dry-run`
  still fetches and verifies but never writes.
- Writes are markdown first, then manifest (see
  `crates/agentforge-core/src/install.rs`), and `RealFs` writes atomically.
- Outputs are deterministic: `GENERATED_AT` is a fixed constant, and the
  bundle, markdown, and manifest are byte-identical for the same input.
- A ruleset tag must equal `RULESET_VERSION`; the workflow asserts this with
  `jq` and fails otherwise.
- Golden rules and the validation pipeline gate every release; never bypass
  them.

## Resume checklist

1. `git checkout main && git pull`, then `gh pr list` and `gh run list` to
   see open work.
2. Run the local verification triplet to confirm the tree is healthy.
3. Read the Outstanding decisions section and resolve what is needed for the
   next task.
4. Pick the first unfinished task in order, create
   `git checkout -b <type>/<short-name>`, implement, verify, open one PR.
5. If a task changes ROADMAP status, tick the checkbox in the same PR.
