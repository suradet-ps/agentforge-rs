use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
  #[error("invalid rule id: {0}")]
  InvalidRuleId(String),

  #[error("duplicate rule id: {0}")]
  DuplicateRuleId(String),

  #[error("duplicate section id: {0}")]
  DuplicateSection(String),

  #[error("override targets nonexistent rule: {0}")]
  OverrideTargetNotFound(String),

  #[error("duplicate override for rule: {0}")]
  DuplicateOverride(String),

  #[error("empty rule set: must contain at least one rule")]
  EmptyRuleSet,

  #[error("missing required field: {0}")]
  MissingField(&'static str),

  #[error("manifest version mismatch: expected {expected}, got {actual}")]
  ManifestVersionMismatch { expected: String, actual: String },

  #[error("failed to parse AGENTS-RUST.md: {0}")]
  Parse(String),
}
