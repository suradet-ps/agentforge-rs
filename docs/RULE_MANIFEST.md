# Rule Manifest Format Specification

Version: 1
Status: Draft

## Overview

A rule manifest is a machine-readable, JSON-serialized companion to the
`AGENTS-RUST.md` markdown file. It allows tooling (CI, the CLI's own
version-check, IDE plugins) to reason about installed rules without parsing
prose.

The manifest is generated from a `RuleSet` via
`RuleManifest::from_rule_set()` and must be deterministic: the same input
produces byte-identical JSON output.

## Schema

```json
{
  "manifest_version": 1,
  "ruleset_version": "0.1.0",
  "generated_at": "2026-07-31T00:00:00Z",
  "rule_count": 2,
  "rules": [
    {
      "id": "5",
      "section": "5",
      "title": "Rust Idioms",
      "severity": "mandatory",
      "tags": ["tokio", "unsafe"],
      "body_checksum": "a1b2c3d4e5f6a1b2"
    }
  ],
  "overrides": [
    {
      "target_rule_id": "5.2",
      "reason": "Use anyhow instead of thiserror for faster iteration"
    }
  ]
}
```

## Fields

### Top-level: `RuleManifest`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `manifest_version` | integer | yes | Schema version of the manifest format. Currently `1`. Forward-compatibility rule: unknown higher versions must be rejected with a typed error. |
| `ruleset_version` | string | yes | Semver of the rule set this manifest describes (e.g. `"0.1.0"`). Independent of the CLI version. |
| `generated_at` | string | yes | ISO-8601 UTC timestamp of when the manifest was generated. |
| `rule_count` | integer | yes | Number of rules in the set. Must equal `rules.len()`. |
| `rules` | array | yes | Ordered list of `ManifestRule` objects. |
| `overrides` | array | yes | Ordered list of `ManifestOverride` objects. May be empty. |

### `ManifestRule`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Rule identifier (e.g. `"5.2"`). Pattern: `[0-9]+(\.[0-9]+)?`. |
| `section` | string | yes | Parent section number (e.g. `"5"` for rule `"5.2"`). |
| `title` | string | yes | Short title of the rule. |
| `severity` | string | yes | One of `"mandatory"`, `"recommended"`, `"advisory"`. |
| `tags` | array[string] | yes | Machine-readable tags. May be empty. |
| `body_checksum` | string | yes | Hex-encoded hash of the rule body text. Used to detect local edits. Algorithm is implementation-defined but must be deterministic. |

### `ManifestOverride`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `target_rule_id` | string | yes | The rule this override applies to. Must match an existing rule ID. |
| `reason` | string | yes | Human-readable reason for the override. Non-empty. |

## Versioning

- The manifest schema version (`manifest_version`) is incremented when the
  schema changes in a backward-incompatible way (field added/removed/renamed,
  type changed).
- The rule set version (`ruleset_version`) follows semver and is independent
  of both the CLI version and the manifest schema version.
- Forward compatibility: a reader that does not recognize a higher
  `manifest_version` must reject the manifest with a
  `ManifestVersionMismatch` error.

## Determinism

The same `RuleSet` input must always produce the same JSON output:

- `rules` are emitted in insertion order (the order they appear in the
  `RuleSet`).
- `body_checksum` is computed from the rule body text using a deterministic
  hash function.
- `generated_at` is provided by the caller, not read from the system clock.
- JSON serialization uses `serde_json::to_string_pretty` with default
  settings (2-space indent).

## Example

Given a `RuleSet` with two rules and one override:

```rust
let mut rs = RuleSet::new("0.1.0".into());
rs.add_rule(Rule::new(
    RuleId::new("5").unwrap(),
    "5".into(),
    "Rust Idioms".into(),
    "Follow idiomatic Rust patterns.".into(),
    Severity::Mandatory,
)).unwrap();
rs.add_rule(Rule::new(
    RuleId::new("5.2").unwrap(),
    "5".into(),
    "Error Handling".into(),
    "Use thiserror for library crates.".into(),
    Severity::Recommended,
)).unwrap();
rs.add_override(Override::new(
    RuleId::new("5.2").unwrap(),
    "Use anyhow".into(),
)).unwrap();
```

The manifest produced by `RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z")`
would be:

```json
{
  "manifest_version": 1,
  "ruleset_version": "0.1.0",
  "generated_at": "2026-01-01T00:00:00Z",
  "rule_count": 2,
  "rules": [
    {
      "id": "5",
      "section": "5",
      "title": "Rust Idioms",
      "severity": "mandatory",
      "tags": [],
      "body_checksum": "<hash of 'Follow idiomatic Rust patterns.'>"
    },
    {
      "id": "5.2",
      "section": "5",
      "title": "Error Handling",
      "severity": "recommended",
      "tags": [],
      "body_checksum": "<hash of 'Use thiserror for library crates.'>"
    }
  ],
  "overrides": [
    {
      "target_rule_id": "5.2",
      "reason": "Use anyhow"
    }
  ]
}
```
