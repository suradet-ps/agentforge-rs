//! Golden-rule gate: fixed invariants the shipped ruleset must never lose.
//!
//! The constitution is a safety artifact: a ruleset that silently drops or
//! weakens a mandatory rule must never ship. `check_golden_rules` verifies
//! that every golden rule still exists with its required content, and the
//! CI gate (`tests/golden_rules.rs`) runs it against the bundled ruleset.

use agentforge_domain::rule::RuleSet;
use agentforge_domain::rule_id::RuleId;
use serde::{Deserialize, Serialize};

/// A golden rule: the rule with `id` must exist and its body must contain
/// every string in `requires`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldenRule {
  /// Rule id, e.g. `"5.2"` (must exist in the ruleset).
  pub id: String,
  /// Substrings the rule body must contain to keep its guarantee.
  pub requires: Vec<String>,
}

/// Check a ruleset against the golden rules, returning every violation.
///
/// A violation is either a missing golden rule or a golden rule whose body
/// no longer contains one of its required strings.
pub fn check_golden_rules(ruleset: &RuleSet, golden: &[GoldenRule]) -> Vec<String> {
  let mut violations = Vec::new();

  for g in golden {
    let id = match RuleId::new(&g.id) {
      Ok(id) => id,
      Err(_) => {
        violations.push(format!("golden rule has an invalid id: {}", g.id));
        continue;
      }
    };

    let Some(rule) = ruleset.get_rule(&id) else {
      violations.push(format!("golden rule §{} is missing from the ruleset", g.id));
      continue;
    };

    for need in &g.requires {
      if !rule.body.contains(need.as_str()) {
        violations.push(format!(
          "golden rule §{} lost required content: {need}",
          g.id
        ));
      }
    }
  }

  violations
}

#[cfg(test)]
mod tests {
  use super::*;
  use agentforge_domain::rule::{Rule, RuleSet, Severity};

  fn make_ruleset() -> RuleSet {
    let mut rs = RuleSet::new("1.0.0".into());
    rs.add_rule(Rule::new(
      RuleId::new("3").unwrap(),
      "3".into(),
      "Checks".into(),
      "cargo clippy --all-targets -- -D warnings".into(),
      Severity::Mandatory,
    ))
    .unwrap();
    rs
  }

  #[test]
  fn clean_ruleset_has_no_violations() {
    let golden = vec![GoldenRule {
      id: "3".into(),
      requires: vec!["clippy".into(), "-D warnings".into()],
    }];
    assert!(check_golden_rules(&make_ruleset(), &golden).is_empty());
  }

  #[test]
  fn missing_rule_is_reported() {
    let golden = vec![GoldenRule {
      id: "9".into(),
      requires: vec!["anything".into()],
    }];
    let violations = check_golden_rules(&make_ruleset(), &golden);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("§9"));
  }

  #[test]
  fn lost_content_is_reported() {
    let golden = vec![GoldenRule {
      id: "3".into(),
      requires: vec!["cargo test".into()],
    }];
    let violations = check_golden_rules(&make_ruleset(), &golden);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("cargo test"));
  }

  #[test]
  fn multiple_violations_are_collected() {
    let golden = vec![
      GoldenRule {
        id: "3".into(),
        requires: vec!["cargo test".into(), "missing-content".into()],
      },
      GoldenRule {
        id: "9".into(),
        requires: vec!["x".into()],
      },
    ];
    let violations = check_golden_rules(&make_ruleset(), &golden);
    assert_eq!(violations.len(), 3);
  }
}
