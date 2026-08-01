use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::rule_id::RuleId;

/// A parsed `[OVERRIDE §X]` directive.
///
/// Overrides let project maintainers deviate from a mandatory rule without
/// modifying the baseline constitution. Each override targets exactly one
/// rule by ID and carries a free-text reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Override {
  /// The rule this override applies to.
  pub target_rule_id: RuleId,
  /// Human-readable reason for the override.
  pub reason: String,
}

impl Override {
  /// Create a new override targeting the given rule.
  pub fn new(target_rule_id: RuleId, reason: String) -> Self {
    Self {
      target_rule_id,
      reason,
    }
  }

  /// Parse an override from a raw line like `"[OVERRIDE §5.2] Use anyhow"`.
  ///
  /// # Errors
  ///
  /// Returns `DomainError::InvalidRuleId` if the rule ID embedded in the
  /// line is malformed.
  pub fn parse_line(line: &str) -> Result<Self, DomainError> {
    let trimmed = line.trim();
    // Expected: [OVERRIDE §<id>] <reason>
    let Some(rest) = trimmed.strip_prefix("[OVERRIDE §") else {
      return Err(DomainError::InvalidRuleId(format!(
        "not an override line: {trimmed}"
      )));
    };
    let Some((id_part, reason)) = rest.split_once(']') else {
      return Err(DomainError::InvalidRuleId(format!(
        "missing closing bracket: {trimmed}"
      )));
    };
    let id = RuleId::new(id_part)?;
    let reason = reason.trim().to_string();
    if reason.is_empty() {
      return Err(DomainError::MissingField("override reason"));
    }
    Ok(Self::new(id, reason))
  }
}

impl fmt::Display for Override {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "[OVERRIDE §{}] {}", self.target_rule_id, self.reason)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_valid() {
    let ovr = Override::parse_line("[OVERRIDE §5.2] Use anyhow").unwrap();
    assert_eq!(ovr.target_rule_id.as_str(), "5.2");
    assert_eq!(ovr.reason, "Use anyhow");
  }

  #[test]
  fn parse_with_whitespace() {
    let ovr = Override::parse_line("  [OVERRIDE §14]  edition 2021  ").unwrap();
    assert_eq!(ovr.target_rule_id.as_str(), "14");
    assert_eq!(ovr.reason, "edition 2021");
  }

  #[test]
  fn parse_not_override() {
    assert!(Override::parse_line("## 5.2 Error Handling").is_err());
  }

  #[test]
  fn parse_missing_bracket() {
    assert!(Override::parse_line("[OVERRIDE §5.2 Use anyhow").is_err());
  }

  #[test]
  fn parse_empty_reason() {
    assert!(Override::parse_line("[OVERRIDE §5.2]").is_err());
  }

  #[test]
  fn parse_bad_id() {
    assert!(Override::parse_line("[OVERRIDE §] reason").is_err());
    assert!(Override::parse_line("[OVERRIDE §abc] reason").is_err());
  }

  #[test]
  fn display() {
    let ovr = Override::new(RuleId::new("5.2").unwrap(), "Use anyhow".into());
    assert_eq!(format!("{ovr}"), "[OVERRIDE §5.2] Use anyhow");
  }
}
