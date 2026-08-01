use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// A validated rule identifier like `"5.2"` or `"14"`.
///
/// Rule IDs follow the pattern `<major>` or `<major>.<minor>` where major
/// is a section number and minor is an optional sub-section number. The ID
/// is always stored as a trimmed, non-empty string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleId(String);

impl PartialOrd for RuleId {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for RuleId {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> {
      s.split('.')
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect()
    };
    parse(&self.0).cmp(&parse(&other.0))
  }
}

impl Serialize for RuleId {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&self.0)
  }
}

impl<'de> Deserialize<'de> for RuleId {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let s = String::deserialize(deserializer)?;
    RuleId::new(&s).map_err(serde::de::Error::custom)
  }
}

impl RuleId {
  /// Parse and validate a rule ID string.
  ///
  /// # Errors
  ///
  /// Returns `DomainError::InvalidRuleId` if the input is empty, contains
  /// only whitespace, or has characters outside `[0-9.]`.
  pub fn new(raw: &str) -> Result<Self, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
      return Err(DomainError::InvalidRuleId(
        "empty or whitespace-only".into(),
      ));
    }
    if !trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') {
      return Err(DomainError::InvalidRuleId(format!(
        "contains invalid characters: {trimmed}"
      )));
    }
    // Reject trailing/leading dots and consecutive dots
    if trimmed.starts_with('.') || trimmed.ends_with('.') || trimmed.contains("..") {
      return Err(DomainError::InvalidRuleId(format!(
        "malformed dot pattern: {trimmed}"
      )));
    }
    Ok(RuleId(trimmed.to_owned()))
  }

  /// Return the raw string representation.
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for RuleId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl AsRef<str> for RuleId {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl TryFrom<&str> for RuleId {
  type Error = DomainError;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Self::new(value)
  }
}

impl TryFrom<String> for RuleId {
  type Error = DomainError;

  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(&value)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn valid_ids() {
    assert_eq!(RuleId::new("5").unwrap().as_str(), "5");
    assert_eq!(RuleId::new("5.2").unwrap().as_str(), "5.2");
    assert_eq!(RuleId::new("14").unwrap().as_str(), "14");
    assert_eq!(RuleId::new("  5.2  ").unwrap().as_str(), "5.2");
  }

  #[test]
  fn invalid_empty() {
    assert!(RuleId::new("").is_err());
    assert!(RuleId::new("   ").is_err());
  }

  #[test]
  fn invalid_characters() {
    assert!(RuleId::new("5a").is_err());
    assert!(RuleId::new("5-2").is_err());
    assert!(RuleId::new("§5").is_err());
  }

  #[test]
  fn invalid_dot_pattern() {
    assert!(RuleId::new(".5").is_err());
    assert!(RuleId::new("5.").is_err());
    assert!(RuleId::new("5..2").is_err());
  }

  #[test]
  fn ordering() {
    let a = RuleId::new("5").unwrap();
    let b = RuleId::new("5.2").unwrap();
    let c = RuleId::new("14").unwrap();
    assert!(a < b);
    assert!(b < c);
  }

  #[test]
  fn try_from_string() {
    let id = RuleId::try_from("1.1".to_string()).unwrap();
    assert_eq!(id.as_str(), "1.1");
  }

  #[test]
  fn display() {
    let id = RuleId::new("5.2").unwrap();
    assert_eq!(format!("{id}"), "5.2");
  }
}
