## LIB-1. Library Crates

### LIB-1.1 Public API Surface
- Keep the public API minimal and intentional; every `pub` item is a
  long-term contract (§6). Prefer `pub(crate)` for internals.
- Export one canonical root path for each concept; avoid near-duplicate
  names (`Builder`, `Config`, `Options` variants) that confuse users.

### LIB-1.2 SemVer Discipline
- Pin and document the MSRV; treat raising it as a breaking change.
- Never break a public API without a major version bump. Additive changes
  (`#[non_exhaustive]`, new fields) go in minor releases.
- Keep a changelog that maps releases to user-visible changes, not just
  internal refactors.

### LIB-1.3 Documentation & Examples
- Document every public item with `///` including error cases; ship a
  runnable `# Examples` block for non-trivial items (§9).
- Document feature flags and their interactions; test the crate with the
  default feature set and with `--no-default-features`.

### LIB-1.4 Dependency Hygiene
- Prefer a small, stable dependency set; avoid pulling the same concern from
  multiple crates. Gate optional functionality behind features.
- Validate `cargo deny` and `cargo audit` in CI before every release; a new
  transitive dependency is a reviewable change, not an accident.