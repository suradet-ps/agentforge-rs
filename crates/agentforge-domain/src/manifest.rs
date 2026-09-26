use serde::{Deserialize, Serialize};
use sha2::Digest;

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

  /// Whether two manifests describe the same effective rule set.
  ///
  /// Compares the ruleset version, rules, and overrides while ignoring
  /// bookkeeping fields (`generated_at`, `manifest_version`, `rule_count`)
  /// that differ between regenerations of the same ruleset. Used to detect
  /// "already installed, nothing to do" without tripping on a fresh
  /// timestamp.
  pub fn content_eq(&self, other: &Self) -> bool {
    self.ruleset_version == other.ruleset_version
      && self.rules == other.rules
      && self.overrides == other.overrides
  }
}

/// SHA-256 hex digest of `input` (lowercase, 64 characters).
fn sha256_hex(input: &str) -> String {
  const HEX: &[u8; 16] = b"0123456789abcdef";
  let digest = sha2::Sha256::digest(input.as_bytes());
  let mut hex = String::with_capacity(digest.len() * 2);
  for byte in digest {
    hex.push(HEX[(byte >> 4) as usize] as char);
    hex.push(HEX[(byte & 0x0f) as usize] as char);
  }
  hex
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
  fn sha256_hex_known_vectors() {
    assert_eq!(
      sha256_hex(""),
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
      sha256_hex("abc"),
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
  }

  #[test]
  fn body_checksum_is_64_char_sha256() {
    let rs = make_ruleset();
    let m = RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap();
    assert_eq!(
      m.rules[0].body_checksum,
      "b4e035751b5a749f4a70c0430b483032b94f83427aa439045c69a7ad6784906c"
    );
    assert!(m.rules.iter().all(|r| r.body_checksum.len() == 64));
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
