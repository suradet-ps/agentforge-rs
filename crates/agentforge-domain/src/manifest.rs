use serde::{Deserialize, Serialize};

/// A machine-readable, versioned companion to the `AGENTS-RUST.md` file.
///
/// Tooling (CI, the CLI's own version-check, IDE plugins) reads this
/// instead of parsing prose. The manifest is generated from the canonical
/// `RuleSet` and must be byte-for-byte reproducible for the same input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleManifest {
  /// Schema version of the manifest format itself.
  pub manifest_version: u32,
  /// Semver of the rule set this manifest describes.
  pub ruleset_version: String,
  /// ISO-8601 timestamp of when the manifest was generated.
  pub generated_at: String,
  /// Number of rules in the set.
  pub rule_count: usize,
  /// The rules, in order.
  pub rules: Vec<ManifestRule>,
  /// Overrides present in the set.
  pub overrides: Vec<ManifestOverride>,
}

/// Compact rule representation for the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestRule {
  /// Rule ID (e.g. `"5.2"`).
  pub id: String,
  /// Parent section number.
  pub section: String,
  /// Short title.
  pub title: String,
  /// Severity label.
  pub severity: String,
  /// Tags.
  pub tags: Vec<String>,
  /// SHA-256 hex digest of the rule body.
  pub body_checksum: String,
}

/// Compact override representation for the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestOverride {
  /// The rule this override targets.
  pub target_rule_id: String,
  /// Reason text.
  pub reason: String,
}

impl RuleManifest {
  /// Build a manifest from a `RuleSet`.
  ///
  /// `generated_at` should be an ISO-8601 timestamp. Body checksums are
  /// computed as SHA-256 hex digests.
  pub fn from_rule_set(
    rs: &crate::rule::RuleSet,
    generated_at: &str,
  ) -> Result<Self, crate::error::DomainError> {
    if rs.rules.is_empty() {
      return Err(crate::error::DomainError::EmptyRuleSet);
    }

    let rules = rs
      .rules
      .iter()
      .map(|r| ManifestRule {
        id: r.id.to_string(),
        section: r.section.clone(),
        title: r.title.clone(),
        severity: r.severity.label().to_string(),
        tags: r.tags.clone(),
        body_checksum: sha256_hex(&r.body),
      })
      .collect();

    let overrides = rs
      .overrides
      .iter()
      .map(|o| ManifestOverride {
        target_rule_id: o.target_rule_id.to_string(),
        reason: o.reason.clone(),
      })
      .collect();

    Ok(Self {
      manifest_version: 1,
      ruleset_version: rs.version.clone(),
      generated_at: generated_at.to_string(),
      rule_count: rs.rules.len(),
      rules,
      overrides,
    })
  }
}

/// Minimal SHA-256 hex digest (no external crate dependency).
///
/// This is a placeholder that produces a deterministic hex string from the
/// input. For production use, swap this with a real SHA-256 crate.
fn sha256_hex(input: &str) -> String {
  // Use a simple FNV-1a-like hash for now; will be replaced with real
  // SHA-256 when we add the `sha2` crate dependency. For the domain
  // model's purposes this is sufficient: it must be deterministic and
  // change when the input changes.
  let bytes = input.as_bytes();
  let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
  for &b in bytes {
    hash ^= b as u64;
    hash = hash.wrapping_mul(0x100000001b3); // FNV prime
  }
  format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::r#override::Override;
  use crate::rule::{Rule, RuleSet, Severity};
  use crate::rule_id::RuleId;

  fn make_ruleset() -> RuleSet {
    let mut rs = RuleSet::new("0.1.0".into());
    rs.add_rule(Rule::new(
      RuleId::new("5").unwrap(),
      "5".into(),
      "Rust Idioms".into(),
      "Follow idiomatic Rust patterns.".into(),
      Severity::Mandatory,
    ))
    .unwrap();
    rs.add_rule(Rule::new(
      RuleId::new("5.2").unwrap(),
      "5".into(),
      "Error Handling".into(),
      "Use thiserror for library crates.".into(),
      Severity::Recommended,
    ))
    .unwrap();
    rs
  }

  #[test]
  fn from_ruleset_ok() {
    let rs = make_ruleset();
    let m = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();
    assert_eq!(m.manifest_version, 1);
    assert_eq!(m.ruleset_version, "0.1.0");
    assert_eq!(m.rule_count, 2);
    assert_eq!(m.rules.len(), 2);
    assert_eq!(m.rules[0].id, "5");
    assert_eq!(m.rules[1].id, "5.2");
  }

  #[test]
  fn from_ruleset_empty() {
    let rs = RuleSet::new("0.1.0".into());
    assert!(RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").is_err());
  }

  #[test]
  fn body_checksum_deterministic() {
    let rs = make_ruleset();
    let m1 = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();
    let m2 = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();
    assert_eq!(m1.rules[0].body_checksum, m2.rules[0].body_checksum);
  }

  #[test]
  fn body_checksum_changes_with_input() {
    let rs1 = make_ruleset();
    let mut rs2 = make_ruleset();
    // Modify body of second rule
    rs2.rules[1].body = "Different body text.".into();
    let m1 = RuleManifest::from_rule_set(&rs1, "2026-01-01T00:00:00Z").unwrap();
    let m2 = RuleManifest::from_rule_set(&rs2, "2026-01-01T00:00:00Z").unwrap();
    assert_ne!(m1.rules[1].body_checksum, m2.rules[1].body_checksum);
  }

  #[test]
  fn sha256_hex_is_deterministic() {
    let a = sha256_hex("hello");
    let b = sha256_hex("hello");
    assert_eq!(a, b);
    let c = sha256_hex("world");
    assert_ne!(a, c);
  }

  #[test]
  fn manifest_serializes() {
    let rs = make_ruleset();
    let m = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();
    let json = serde_json::to_string_pretty(&m).unwrap();
    assert!(json.contains("\"manifest_version\": 1"));
    assert!(json.contains("\"ruleset_version\": \"0.1.0\""));
  }

  #[test]
  fn round_trip() {
    // 1. Build a RuleSet
    let mut rs = RuleSet::new("1.0.0".into());
    rs.add_rule(Rule::new(
      RuleId::new("0").unwrap(),
      "0".into(),
      "Golden Rules".into(),
      "These rules always apply.".into(),
      Severity::Mandatory,
    ))
    .unwrap();
    rs.add_rule(Rule::new(
      RuleId::new("5.2").unwrap(),
      "5".into(),
      "Error Handling".into(),
      "Use thiserror for library crates, anyhow for binaries.".into(),
      Severity::Recommended,
    ))
    .unwrap();
    rs.add_override(Override::new(
      RuleId::new("5.2").unwrap(),
      "Use anyhow for faster iteration.".into(),
    ))
    .unwrap();

    // 2. Generate manifest
    let m1 = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();

    // 3. Serialize to JSON
    let json = serde_json::to_string(&m1).unwrap();

    // 4. Deserialize back
    let m2: RuleManifest = serde_json::from_str(&json).unwrap();

    // 5. Verify identical
    assert_eq!(m1, m2);
    assert_eq!(m2.manifest_version, 1);
    assert_eq!(m2.ruleset_version, "1.0.0");
    assert_eq!(m2.rule_count, 2);
    assert_eq!(m2.rules.len(), 2);
    assert_eq!(m2.overrides.len(), 1);
    assert_eq!(m2.rules[0].id, "0");
    assert_eq!(m2.rules[1].id, "5.2");
    assert_eq!(m2.overrides[0].target_rule_id, "5.2");
    assert_eq!(m2.overrides[0].reason, "Use anyhow for faster iteration.");
  }

  #[test]
  fn round_trip_empty_overrides() {
    let mut rs = RuleSet::new("0.1.0".into());
    rs.add_rule(Rule::new(
      RuleId::new("1").unwrap(),
      "1".into(),
      "Agent Behavior".into(),
      "Think before acting.".into(),
      Severity::Mandatory,
    ))
    .unwrap();

    let m1 = RuleManifest::from_rule_set(&rs, "2026-06-15T12:00:00Z").unwrap();
    let json = serde_json::to_string_pretty(&m1).unwrap();
    let m2: RuleManifest = serde_json::from_str(&json).unwrap();

    assert_eq!(m1, m2);
    assert!(m2.overrides.is_empty());
  }

  #[test]
  fn manifest_reproducible() {
    // Same input, same timestamp → byte-identical JSON
    let rs = make_ruleset();
    let m1 = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();
    let m2 = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();
    let json1 = serde_json::to_string(&m1).unwrap();
    let json2 = serde_json::to_string(&m2).unwrap();
    assert_eq!(json1, json2);
  }
}
