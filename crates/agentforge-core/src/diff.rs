//! Rule-level diffing between two manifests.

use agentforge_domain::manifest::RuleManifest;
use serde::Serialize;

/// How a rule changed between the installed and target rulesets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Change {
  /// Present and byte-identical in both.
  Unchanged,
  /// Present in both but the body differs (a local edit).
  Edited,
  /// In the target but missing from the installed ruleset.
  Added,
  /// In the installed ruleset but absent from the target.
  Removed,
}

/// One changed rule, with counts summarising the whole report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleDiff {
  /// Rule id (e.g. `5.2`, `WASM-1.1`).
  pub id: String,
  /// Rule title from the manifest that introduced it.
  pub title: String,
  /// How it changed.
  pub change: Change,
}

/// Summary counts plus the individual changed rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffReport {
  pub unchanged: usize,
  pub edited: usize,
  pub added: usize,
  pub removed: usize,
  /// Only the changed rules (`Edited` / `Added` / `Removed`), in target
  /// order followed by installed-only rules.
  pub rules: Vec<RuleDiff>,
}

/// Compare an installed ruleset against a target (bundled) one, rule by
/// rule, using body checksums. Local edits show up as `Edited`.
pub fn diff_manifests(installed: &RuleManifest, target: &RuleManifest) -> DiffReport {
  let mut rules: Vec<RuleDiff> = Vec::new();
  let mut unchanged = 0usize;
  let mut edited = 0usize;
  let mut added = 0usize;

  for t in &target.rules {
    match installed.rules.iter().find(|r| r.id == t.id) {
      None => {
        added += 1;
        rules.push(RuleDiff {
          id: t.id.clone(),
          title: t.title.clone(),
          change: Change::Added,
        });
      }
      Some(i) if i.body_checksum != t.body_checksum => {
        edited += 1;
        rules.push(RuleDiff {
          id: t.id.clone(),
          title: t.title.clone(),
          change: Change::Edited,
        });
      }
      Some(_) => {
        unchanged += 1;
      }
    }
  }

  let mut removed = 0usize;
  for i in &installed.rules {
    if !target.rules.iter().any(|r| r.id == i.id) {
      removed += 1;
      rules.push(RuleDiff {
        id: i.id.clone(),
        title: i.title.clone(),
        change: Change::Removed,
      });
    }
  }

  DiffReport {
    unchanged,
    edited,
    added,
    removed,
    rules,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use agentforge_domain::rule::{Rule, RuleSet, Severity};
  use agentforge_domain::rule_id::RuleId;

  fn manifest(version: &str, edits: &[(&str, &str)]) -> RuleManifest {
    let mut rs = RuleSet::new(version.into());
    for (id, body) in edits {
      let mut rule = Rule::new(
        RuleId::new(id).unwrap(),
        id.split('.').next().unwrap().to_string(),
        format!("Rule {id}"),
        body.to_string(),
        Severity::Mandatory,
      );
      if id.contains('.') {
        rule.section = id.split('.').next().unwrap().to_string();
      }
      rs.add_rule(rule).unwrap();
    }
    RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap()
  }

  #[test]
  fn identical_manifests_are_unchanged() {
    let a = manifest("1.0.0", &[("5", "a"), ("5.2", "b")]);
    let report = diff_manifests(&a, &a);
    assert_eq!(report.unchanged, 2);
    assert_eq!(report.edited, 0);
    assert_eq!(report.added, 0);
    assert_eq!(report.removed, 0);
    assert!(report.rules.is_empty());
  }

  #[test]
  fn detects_local_edit_via_checksum() {
    let installed = manifest("1.0.0", &[("5.2", "edited body")]);
    let target = manifest("1.0.0", &[("5.2", "pristine body")]);
    let report = diff_manifests(&installed, &target);
    assert_eq!(report.edited, 1);
    assert_eq!(report.rules[0].id, "5.2");
    assert_eq!(report.rules[0].change, Change::Edited);
  }

  #[test]
  fn detects_added_and_removed() {
    let installed = manifest("1.0.0", &[("5", "x"), ("7", "extra")]);
    let target = manifest("1.0.0", &[("5", "x"), ("6", "new")]);
    let report = diff_manifests(&installed, &target);
    assert_eq!(report.added, 1); // 6 missing from installed
    assert_eq!(report.removed, 1); // 7 extra in installed
    assert_eq!(report.unchanged, 1); // 5
    assert!(
      report
        .rules
        .iter()
        .any(|r| r.id == "6" && r.change == Change::Added)
    );
    assert!(
      report
        .rules
        .iter()
        .any(|r| r.id == "7" && r.change == Change::Removed)
    );
  }

  #[test]
  fn report_serializes() {
    let installed = manifest("1.0.0", &[("5", "a")]);
    let target = manifest("1.0.0", &[("5", "b"), ("6", "c")]);
    let json = serde_json::to_string(&diff_manifests(&installed, &target)).unwrap();
    assert!(json.contains("\"edited\""));
    assert!(json.contains("\"added\""));
  }
}
