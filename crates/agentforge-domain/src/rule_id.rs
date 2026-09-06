use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// A validated rule identifier like `"5.2"`, `"14"`, or `"WASM-1.2"`.
///
/// Two shapes are accepted:
///
/// - Numeric: `<major>` or `<major>.<minor>` (e.g. `5`, `5.2`, `14`).
/// - Namespaced: `<NS>-<n>` or `<NS>-<n>.<m>` where `<NS>` is an all-alpha
///   domain tag (e.g. `WASM-1`, `WASM-1.2`). Used by domain template
///   fragments so they own a distinct id space that cannot collide with the
///   numeric core constitution.
///
/// Ordering is stable: numeric ids sort before namespaced ids, and within a
/// namespace ids sort numerically (`WASM-1.2` < `WASM-1.10`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleId(String);

impl PartialOrd for RuleId {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for RuleId {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.key().cmp(&other.key())
  }
}

impl RuleId {
  /// Return a sortable key: `(namespace, numeric-parts)`.
  ///
  /// Numeric ids have an empty namespace; namespaced ids carry their tag.
  fn key(&self) -> (String, Vec<u32>) {
    if let Some((ns, rest)) = self.0.split_once('-') {
      let nums = rest
        .split('.')
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect();
      (ns.to_string(), nums)
    } else {
      let nums = self
        .0
        .split('.')
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect();
      (String::new(), nums)
    }
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
  /// Accepts numeric ids (`5`, `5.2`) and namespaced ids (`WASM-1`,
  /// `WASM-1.2`).
  ///
  /// # Errors
  ///
  /// Returns `DomainError::InvalidRuleId` for malformed input.
  pub fn new(raw: &str) -> Result<Self, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
      return Err(DomainError::InvalidRuleId(
        "empty or whitespace-only".into(),
      ));
    }
    if !trimmed
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
      return Err(DomainError::InvalidRuleId(format!(
        "contains invalid characters: {trimmed}"
      )));
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') || trimmed.contains("..") {
      return Err(DomainError::InvalidRuleId(format!(
        "malformed dot pattern: {trimmed}"
      )));
    }
    if trimmed.starts_with('-')
      || trimmed.ends_with('-')
      || trimmed.contains("--")
      || trimmed.contains(".-")
      || trimmed.contains("-.")
    {
      return Err(DomainError::InvalidRuleId(format!(
        "malformed dash pattern: {trimmed}"
      )));
    }

    if let Some((ns, rest)) = trimmed.split_once('-') {
      if ns.is_empty() || !ns.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(DomainError::InvalidRuleId(format!(
          "namespace must be all letters: {trimmed}"
        )));
      }
      if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(DomainError::InvalidRuleId(format!(
          "namespaced id must be `<NS>-<digits>`: {trimmed}"
        )));
      }
    } else if !trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') {
      return Err(DomainError::InvalidRuleId(format!(
        "numeric id must contain only digits and dots: {trimmed}"
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
  fn valid_namespaced_ids() {
    assert_eq!(RuleId::new("WASM-1").unwrap().as_str(), "WASM-1");
    assert_eq!(RuleId::new("WASM-1.2").unwrap().as_str(), "WASM-1.2");
    assert_eq!(RuleId::new("wasm-3").unwrap().as_str(), "wasm-3");
  }

  #[test]
  fn invalid_namespaced_ids() {
    assert!(RuleId::new("WASM-").is_err()); // empty number
    assert!(RuleId::new("-1").is_err()); // empty namespace
    assert!(RuleId::new("WASM1-2").is_err()); // digit in namespace
    assert!(RuleId::new("WASM-1-2").is_err()); // two dashes
    assert!(RuleId::new("WASM--1").is_err()); // consecutive dashes
    assert!(RuleId::new("WASM-.1").is_err()); // dash before dot
    assert!(RuleId::new("WASM-1.").is_err()); // trailing dot
  }

  #[test]
  fn namespaced_ordering() {
    assert!(RuleId::new("WASM-1").unwrap() < RuleId::new("WASM-2").unwrap());
    assert!(RuleId::new("WASM-1.2").unwrap() < RuleId::new("WASM-1.10").unwrap());
    assert!(RuleId::new("WASM-9").unwrap() < RuleId::new("WASM-10").unwrap());
    // numeric core ids sort before namespaced ids
    assert!(RuleId::new("14").unwrap() < RuleId::new("WASM-1").unwrap());
    // namespaces order lexicographically
    assert!(RuleId::new("TAURI-1").unwrap() < RuleId::new("WASM-1").unwrap());
  }

  #[test]
  fn numeric_ids_still_reject_letters_and_dashes() {
    assert!(RuleId::new("5a").is_err());
    assert!(RuleId::new("5-2").is_err());
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
