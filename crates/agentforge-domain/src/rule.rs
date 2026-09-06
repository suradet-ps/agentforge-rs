use std::fmt;

use serde::{Deserialize, Serialize};

/// How binding a rule is for AI agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
  /// Breaking this rule is always wrong.
  Mandatory,
  /// Strong recommendation; deviation requires explicit override.
  Recommended,
  /// Guidance that can be freely overridden.
  Advisory,
}

impl PartialOrd for Severity {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Severity {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    fn rank(s: Severity) -> u8 {
      match s {
        Severity::Advisory => 0,
        Severity::Recommended => 1,
        Severity::Mandatory => 2,
      }
    }
    rank(*self).cmp(&rank(*other))
  }
}

impl Severity {
  /// Return a human-readable label.
  pub fn label(self) -> &'static str {
    match self {
      Severity::Mandatory => "mandatory",
      Severity::Recommended => "recommended",
      Severity::Advisory => "advisory",
    }
  }
}

impl fmt::Display for Severity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.label())
  }
}

impl std::str::FromStr for Severity {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "mandatory" => Ok(Severity::Mandatory),
      "recommended" => Ok(Severity::Recommended),
      "advisory" => Ok(Severity::Advisory),
      _ => Err(format!("unknown severity: {s}")),
    }
  }
}

/// A single rule within the constitution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
  /// Identifier like `"5.2"`.
  pub id: crate::rule_id::RuleId,
  /// Parent section this rule belongs to (e.g. `"5"` for `"5.2"`).
  pub section: String,
  /// Short title of the rule.
  pub title: String,
  /// Full rule body text.
  pub body: String,
  /// How binding this rule is.
  pub severity: Severity,
  /// Machine-readable tags (e.g. `"tokio"`, `"unsafe"`, `"testing"`).
  pub tags: Vec<String>,
}

impl Rule {
  /// Create a new rule with no tags.
  pub fn new(
    id: crate::rule_id::RuleId,
    section: String,
    title: String,
    body: String,
    severity: Severity,
  ) -> Self {
    Self {
      id,
      section,
      title,
      body,
      severity,
      tags: Vec::new(),
    }
  }

  /// Check whether this rule carries a specific tag.
  pub fn has_tag(&self, tag: &str) -> bool {
    self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
  }
}

/// A section heading (`## N. Title`), carrying the title that prose rules
/// do not. Needed to render a `RuleSet` back to faithful markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
  /// Section id (e.g. `5` or `WASM-1`).
  pub id: crate::rule_id::RuleId,
  /// Human-readable section title.
  pub title: String,
}

/// A complete set of rules parsed from `AGENTS-RUST.md`.
#[derive(Debug, Clone)]
pub struct RuleSet {
  /// Version of the rule set (semver string).
  pub version: String,
  /// Ordered rules, keyed by ID for fast lookup.
  pub rules: Vec<Rule>,
  /// Overrides applied on top of the base rules.
  pub overrides: Vec<crate::r#override::Override>,
  /// Ordered section headings seen while parsing.
  pub sections: Vec<Section>,
}

impl RuleSet {
  /// Create an empty rule set.
  pub fn new(version: String) -> Self {
    Self {
      version,
      rules: Vec::new(),
      overrides: Vec::new(),
      sections: Vec::new(),
    }
  }

  /// Add a section heading. Returns an error on duplicate section ID.
  pub fn add_section(&mut self, section: Section) -> Result<(), crate::error::DomainError> {
    if self.sections.iter().any(|s| s.id == section.id) {
      return Err(crate::error::DomainError::DuplicateSection(
        section.id.to_string(),
      ));
    }
    self.sections.push(section);
    Ok(())
  }

  /// Look up a section heading by ID.
  pub fn get_section(&self, id: &crate::rule_id::RuleId) -> Option<&Section> {
    self.sections.iter().find(|s| &s.id == id)
  }

  /// Add a rule. Returns an error on duplicate ID.
  pub fn add_rule(&mut self, rule: Rule) -> Result<(), crate::error::DomainError> {
    if self.rules.iter().any(|r| r.id == rule.id) {
      return Err(crate::error::DomainError::DuplicateRuleId(
        rule.id.to_string(),
      ));
    }
    self.rules.push(rule);
    Ok(())
  }

  /// Add an override. Returns an error if the target rule does not exist
  /// or if there is already an override for the same rule.
  pub fn add_override(
    &mut self,
    ovr: crate::r#override::Override,
  ) -> Result<(), crate::error::DomainError> {
    if !self.rules.iter().any(|r| r.id == ovr.target_rule_id) {
      return Err(crate::error::DomainError::OverrideTargetNotFound(
        ovr.target_rule_id.to_string(),
      ));
    }
    if self
      .overrides
      .iter()
      .any(|o| o.target_rule_id == ovr.target_rule_id)
    {
      return Err(crate::error::DomainError::DuplicateOverride(
        ovr.target_rule_id.to_string(),
      ));
    }
    self.overrides.push(ovr);
    Ok(())
  }

  /// Look up a rule by ID.
  pub fn get_rule(&self, id: &crate::rule_id::RuleId) -> Option<&Rule> {
    self.rules.iter().find(|r| &r.id == id)
  }

  /// Look up an override by target rule ID.
  pub fn get_override(&self, id: &crate::rule_id::RuleId) -> Option<&crate::r#override::Override> {
    self.overrides.iter().find(|o| &o.target_rule_id == id)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::rule_id::RuleId;

  fn make_rule(id: &str) -> Rule {
    Rule::new(
      RuleId::new(id).unwrap(),
      id.split('.').next().unwrap().to_string(),
      format!("Rule {id}"),
      format!("Body of rule {id}"),
      Severity::Mandatory,
    )
  }

  #[test]
  fn severity_ordering() {
    assert!(Severity::Mandatory > Severity::Recommended);
    assert!(Severity::Recommended > Severity::Advisory);
  }

  #[test]
  fn severity_label() {
    assert_eq!(Severity::Mandatory.label(), "mandatory");
    assert_eq!(Severity::Advisory.label(), "advisory");
  }

  #[test]
  fn severity_from_str() {
    assert_eq!(
      "Mandatory".parse::<Severity>().unwrap(),
      Severity::Mandatory
    );
    assert_eq!(
      "recommended".parse::<Severity>().unwrap(),
      Severity::Recommended
    );
    assert!("invalid".parse::<Severity>().is_err());
  }

  #[test]
  fn add_rule_ok() {
    let mut rs = RuleSet::new("0.1.0".into());
    rs.add_rule(make_rule("5")).unwrap();
    assert_eq!(rs.rules.len(), 1);
  }

  #[test]
  fn add_rule_duplicate() {
    let mut rs = RuleSet::new("0.1.0".into());
    rs.add_rule(make_rule("5")).unwrap();
    assert!(rs.add_rule(make_rule("5")).is_err());
  }

  #[test]
  fn add_override_missing_target() {
    let mut rs = RuleSet::new("0.1.0".into());
    let ovr = crate::r#override::Override::new(RuleId::new("5").unwrap(), "use anyhow".into());
    assert!(rs.add_override(ovr).is_err());
  }

  #[test]
  fn add_override_ok() {
    let mut rs = RuleSet::new("0.1.0".into());
    rs.add_rule(make_rule("5")).unwrap();
    let ovr = crate::r#override::Override::new(RuleId::new("5").unwrap(), "use anyhow".into());
    rs.add_override(ovr).unwrap();
    assert_eq!(rs.overrides.len(), 1);
  }

  #[test]
  fn add_override_duplicate() {
    let mut rs = RuleSet::new("0.1.0".into());
    rs.add_rule(make_rule("5")).unwrap();
    let ovr1 = crate::r#override::Override::new(RuleId::new("5").unwrap(), "use anyhow".into());
    let ovr2 = crate::r#override::Override::new(RuleId::new("5").unwrap(), "use eyre".into());
    rs.add_override(ovr1).unwrap();
    assert!(rs.add_override(ovr2).is_err());
  }

  #[test]
  fn get_rule() {
    let mut rs = RuleSet::new("0.1.0".into());
    rs.add_rule(make_rule("5")).unwrap();
    assert!(rs.get_rule(&RuleId::new("5").unwrap()).is_some());
    assert!(rs.get_rule(&RuleId::new("6").unwrap()).is_none());
  }

  #[test]
  fn rule_has_tag() {
    let mut rule = make_rule("5");
    rule.tags.push("tokio".into());
    assert!(rule.has_tag("tokio"));
    assert!(rule.has_tag("Tokio"));
    assert!(!rule.has_tag("async"));
  }
}
