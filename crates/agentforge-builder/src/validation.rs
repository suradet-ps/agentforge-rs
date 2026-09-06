//! Validation pipeline: a ruleset build that hits errors must never ship a
//! manifest. `validation_report` runs a build and reports structured
//! findings (errors, warnings, counts) instead of failing silently.

use agentforge_domain::rule::Severity;
use serde::Serialize;

use crate::{BuildConfig, build};

/// Structured result of a ruleset build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildValidationReport {
  /// Build-blocking problems (parse errors, rule-id collisions, orphan
  /// overrides). Non-empty means **no** manifest may be shipped.
  pub errors: Vec<String>,
  /// Non-blocking observations (e.g. an override weakens a `Mandatory`
  /// rule — allowed by the override system, but noteworthy).
  pub warnings: Vec<String>,
  /// Number of rules in the composed ruleset (0 when the build failed).
  pub rule_count: usize,
  /// Number of domain fragments composed.
  pub fragment_count: usize,
}

/// Run the build and produce a validation report.
///
/// A failing build is captured as `errors` (never a panic): the caller can
/// treat `errors.is_empty()` as "this ruleset is shippable".
pub fn validation_report(cfg: &BuildConfig) -> BuildValidationReport {
  let fragment_count = cfg.fragments.len();

  match build(cfg) {
    Ok(out) => {
      let warnings = out
        .ruleset
        .overrides
        .iter()
        .filter_map(|ovr| out.ruleset.get_rule(&ovr.target_rule_id).map(|r| (ovr, r)))
        .filter(|(_, rule)| rule.severity == Severity::Mandatory)
        .map(|(ovr, _)| {
          format!(
            "override weakens Mandatory rule §{}: {}",
            ovr.target_rule_id, ovr.reason
          )
        })
        .collect();

      BuildValidationReport {
        errors: Vec::new(),
        warnings,
        rule_count: out.ruleset.rules.len(),
        fragment_count,
      }
    }
    Err(e) => BuildValidationReport {
      errors: vec![e.to_string()],
      warnings: Vec::new(),
      rule_count: 0,
      fragment_count,
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const CORE: &str = r#"## 0. Golden Rules

- Always apply.

### 5.2 Errors

- Use thiserror.

[OVERRIDE §0] Golden rules still hold.
"#;

  const WASM: &str = r#"## WASM-1. WebAssembly

### WASM-1.1 Memory

- Keep linear memory.
"#;

  const COLLIDES: &str = r#"## WASM-1. WebAssembly

### WASM-1.1 Memory

- Duplicate.
"#;

  const ORPHAN: &str = r#"## WASM-1. WebAssembly

### WASM-1.1 Memory

- Body.

[OVERRIDE §999.1] No such rule.
"#;

  fn config<'a>(fragments: &'a [(&'a str, &'a str)]) -> BuildConfig<'a> {
    BuildConfig {
      core_template: CORE,
      fragments,
      version: "1.0.0",
      generated_at: "2026-01-01T00:00:00Z",
    }
  }

  #[test]
  fn clean_build_has_zero_errors_and_counts() {
    let report = validation_report(&config(&[]));
    assert!(report.errors.is_empty());
    assert_eq!(report.rule_count, 2);
    assert_eq!(report.fragment_count, 0);
  }

  #[test]
  fn warning_for_override_weakening_mandatory_rule() {
    let report = validation_report(&config(&[]));
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("§0"));
  }

  #[test]
  fn composed_build_reports_fragment_count() {
    let frags = [("wasm", WASM)];
    let report = validation_report(&config(&frags));
    assert!(report.errors.is_empty());
    assert_eq!(report.fragment_count, 1);
    assert_eq!(report.rule_count, 3);
  }

  #[test]
  fn collision_prevents_shippable_build() {
    let frags = [("wasm", WASM), ("collides", COLLIDES)];
    let report = validation_report(&config(&frags));
    assert_eq!(report.errors.len(), 1);
    assert!(!report.errors[0].is_empty());
    assert_eq!(report.rule_count, 0);
  }

  #[test]
  fn orphan_override_prevents_shippable_build() {
    let frags = [("wasm", ORPHAN)];
    let report = validation_report(&config(&frags));
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.rule_count, 0);
  }

  #[test]
  fn report_serializes() {
    let json = serde_json::to_string(&validation_report(&config(&[]))).unwrap();
    assert!(json.contains("\"errors\""));
    assert!(json.contains("\"warnings\""));
    assert!(json.contains("\"rule_count\""));
    assert!(json.contains("\"fragment_count\""));
  }
}
