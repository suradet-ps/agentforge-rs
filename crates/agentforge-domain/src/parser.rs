//! Markdown → [`RuleSet`] parser for `AGENTS-RUST.md`.
//!
//! The constitution is authored as a markdown document, but every downstream
//! feature (versioning, diffing, safe updates, validation) needs a typed
//! model. This module turns the human-readable document into the domain
//! model without ever panicking: every malformed input produces a typed
//! [`DomainError`]. The same scan powers `validate_agents_md`, which reports
//! *every* problem with line numbers instead of stopping at the first.

use serde::Serialize;

use crate::error::DomainError;
use crate::r#override::Override;
use crate::rule::{Rule, RuleSet, Section, Severity};
use crate::rule_id::RuleId;

/// Parse an `AGENTS-RUST.md` document into a validated [`RuleSet`].
///
/// Recognized structure (all heading detection is code-fence aware):
///
/// - `## <n>. <Title>` — a section, where `<n>` is a plain integer (`5`,
///   `14`) or a namespaced id (`WASM-1`) used by domain fragments. Content
///   under a section that has no `###` sub-headings becomes a single rule
///   with id `<n>`.
/// - `### <n>.<m> <Title>` — a sub-rule belonging to the enclosing section.
/// - `[OVERRIDE §<id>] <reason>` — an override directive; the target rule
///   must exist in the same document.
///
/// Section headings are also recorded on the [`RuleSet`] so the document
/// can be rendered back faithfully.
///
/// Anything before the first section heading (e.g. the document preamble)
/// is ignored. Horizontal rules (`---`) are dropped from rule bodies.
///
/// # Errors
///
/// Returns [`DomainError::InvalidRuleId`] for malformed headings,
/// [`DomainError::OverrideTargetNotFound`] / [`DomainError::DuplicateOverride`]
/// for invalid override directives, [`DomainError::EmptyRuleSet`] when no
/// rules are present, and [`DomainError::Parse`] for structural problems.
pub fn parse_agents_md(source: &str, version: &str) -> Result<RuleSet, DomainError> {
  let (collected, issues) = collect(source, true);
  if let Some(issue) = issues.into_iter().next() {
    return Err(issue.err);
  }
  build_ruleset(version, &collected, true)
}

/// Parse a domain fragment where `[OVERRIDE §X]` targets may reference rules
/// in the *composed* set (core + other fragments) rather than the fragment
/// alone. Override targets are therefore **not** validated here; the caller
/// (the builder) validates them after merging.
pub fn parse_agents_md_fragment(source: &str, version: &str) -> Result<RuleSet, DomainError> {
  let (collected, issues) = collect(source, false);
  if let Some(issue) = issues.into_iter().next() {
    return Err(issue.err);
  }
  build_ruleset(version, &collected, false)
}

/// A problem found while validating an `AGENTS-RUST.md` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationIssue {
  /// 1-based line number in the source document.
  pub line: usize,
  /// Category of the problem.
  pub kind: ValidationIssueKind,
  /// Human-readable description.
  pub message: String,
}

/// Category of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ValidationIssueKind {
  /// No rules were found at all.
  EmptyRuleSet,
  /// A section or rule heading is malformed.
  MalformedHeading,
  /// The same rule id appears twice.
  DuplicateRuleId,
  /// The same section id appears twice.
  DuplicateSection,
  /// Two overrides target the same rule.
  DuplicateOverride,
  /// An override line does not match `[OVERRIDE §X] reason`.
  MalformedOverride,
  /// An override targets a rule id that does not exist in the document.
  OrphanOverride,
}

/// The result of validating an `AGENTS-RUST.md` document. Unlike parsing,
/// validation reports every issue it finds instead of stopping at the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
  /// Number of rules successfully recognized.
  pub rule_count: usize,
  /// Total number of issues found.
  pub issue_count: usize,
  /// All issues, in line order.
  pub issues: Vec<ValidationIssue>,
}

/// Validate an `AGENTS-RUST.md` document, collecting every issue with its
/// line number. Never fails: a document with no rules is reported as an
/// [`ValidationIssueKind::EmptyRuleSet`] issue, not an error.
pub fn validate_agents_md(source: &str) -> ValidationReport {
  let (collected, raw) = collect(source, true);
  let issues: Vec<ValidationIssue> = raw
    .into_iter()
    .map(|r| ValidationIssue {
      line: r.line,
      kind: r.kind,
      message: r.err.to_string(),
    })
    .collect();
  ValidationReport {
    rule_count: collected.rules.len(),
    issue_count: issues.len(),
    issues,
  }
}

struct RawIssue {
  line: usize,
  kind: ValidationIssueKind,
  err: DomainError,
}

struct Collected {
  rules: Vec<(Rule, usize)>,
  sections: Vec<(Section, usize)>,
  overrides: Vec<(Override, usize)>,
}

struct Pending {
  id: RuleId,
  section: String,
  title: String,
  body: Vec<String>,
  line: usize,
}

/// Scan a document into collected items plus every issue encountered.
/// Never fails; problems become [`RawIssue`]s.
fn collect(source: &str, validate_overrides: bool) -> (Collected, Vec<RawIssue>) {
  let mut rules: Vec<(Rule, usize)> = Vec::new();
  let mut sections: Vec<(Section, usize)> = Vec::new();
  let mut overrides: Vec<(Override, usize)> = Vec::new();
  let mut issues: Vec<RawIssue> = Vec::new();

  let mut pending: Option<Pending> = None;
  let mut current_section: Option<String> = None;
  let mut in_code_fence = false;

  for (idx, line) in source.lines().enumerate() {
    let lineno = idx + 1;
    let trimmed = line.trim();

    if trimmed.starts_with("```") {
      in_code_fence = !in_code_fence;
      if let Some(p) = &mut pending {
        p.body.push(line.to_string());
      }
      continue;
    }

    if in_code_fence {
      if let Some(p) = &mut pending {
        p.body.push(line.to_string());
      }
      continue;
    }

    if trimmed == "---" {
      continue;
    }

    if trimmed.starts_with("## ") {
      flush_pending(&mut pending, &mut rules, &mut issues);
      match parse_section_heading(trimmed) {
        Err(err) => issues.push(RawIssue {
          line: lineno,
          kind: ValidationIssueKind::MalformedHeading,
          err,
        }),
        Ok((id, title)) => {
          let id_str = id.as_str().to_string();
          if sections.iter().any(|(s, _)| s.id == id) {
            issues.push(RawIssue {
              line: lineno,
              kind: ValidationIssueKind::DuplicateSection,
              err: DomainError::DuplicateSection(id_str.clone()),
            });
          } else {
            sections.push((
              Section {
                id: id.clone(),
                title: title.clone(),
              },
              lineno,
            ));
          }
          current_section = Some(id_str.clone());
          pending = Some(Pending {
            id,
            section: id_str,
            title,
            body: Vec::new(),
            line: lineno,
          });
        }
      }
      continue;
    }

    if trimmed.starts_with("### ") {
      flush_pending(&mut pending, &mut rules, &mut issues);
      match parse_rule_heading(trimmed) {
        Err(err) => issues.push(RawIssue {
          line: lineno,
          kind: ValidationIssueKind::MalformedHeading,
          err,
        }),
        Ok((id, title)) => {
          let section = current_section.clone().unwrap_or_else(|| {
            id.as_str()
              .split('.')
              .next()
              .unwrap_or_default()
              .to_string()
          });
          pending = Some(Pending {
            id,
            section,
            title,
            body: Vec::new(),
            line: lineno,
          });
        }
      }
      continue;
    }

    if trimmed.starts_with("[OVERRIDE") {
      match Override::parse_line(trimmed) {
        Err(err) => issues.push(RawIssue {
          line: lineno,
          kind: ValidationIssueKind::MalformedOverride,
          err,
        }),
        Ok(ovr) => {
          if overrides
            .iter()
            .any(|(o, _)| o.target_rule_id == ovr.target_rule_id)
          {
            issues.push(RawIssue {
              line: lineno,
              kind: ValidationIssueKind::DuplicateOverride,
              err: DomainError::DuplicateOverride(ovr.target_rule_id.to_string()),
            });
          } else {
            overrides.push((ovr, lineno));
          }
        }
      }
      continue;
    }

    if let Some(p) = &mut pending {
      p.body.push(line.to_string());
    }
  }

  flush_pending(&mut pending, &mut rules, &mut issues);

  if rules.is_empty() {
    issues.push(RawIssue {
      line: 1,
      kind: ValidationIssueKind::EmptyRuleSet,
      err: DomainError::EmptyRuleSet,
    });
  }

  if validate_overrides {
    for (ovr, line) in &overrides {
      if !rules.iter().any(|(r, _)| r.id == ovr.target_rule_id) {
        issues.push(RawIssue {
          line: *line,
          kind: ValidationIssueKind::OrphanOverride,
          err: DomainError::OverrideTargetNotFound(ovr.target_rule_id.to_string()),
        });
      }
    }
  }

  (
    Collected {
      rules,
      sections,
      overrides,
    },
    issues,
  )
}

/// Finalize the current pending rule into the rules list, detecting
/// duplicate rule ids. Emits a [`RawIssue`] for duplicates instead of
/// failing.
fn flush_pending(
  pending: &mut Option<Pending>,
  rules: &mut Vec<(Rule, usize)>,
  issues: &mut Vec<RawIssue>,
) {
  let Some(p) = pending.take() else {
    return;
  };
  let body = clean_body(&p.body);
  if body.is_empty() {
    return;
  }
  if rules.iter().any(|(r, _)| r.id == p.id) {
    issues.push(RawIssue {
      line: p.line,
      kind: ValidationIssueKind::DuplicateRuleId,
      err: DomainError::DuplicateRuleId(p.id.to_string()),
    });
    return;
  }
  rules.push((
    Rule::new(p.id, p.section, p.title, body, Severity::Mandatory),
    p.line,
  ));
}

fn build_ruleset(
  version: &str,
  collected: &Collected,
  validate_overrides: bool,
) -> Result<RuleSet, DomainError> {
  let mut rs = RuleSet::new(version.to_string());
  for (section, _) in &collected.sections {
    rs.add_section(section.clone())?;
  }
  for (rule, _) in &collected.rules {
    rs.add_rule(rule.clone())?;
  }
  for (ovr, _) in &collected.overrides {
    if validate_overrides {
      rs.add_override(ovr.clone())?;
    } else {
      rs.overrides.push(ovr.clone());
    }
  }
  Ok(rs)
}

/// Split a section heading `## <id>. <Title>` into `(id, title)`.
///
/// Section ids are either plain integers (`5`, `14`) or namespaced ids
/// without a sub-number (`WASM-1`).
fn parse_section_heading(line: &str) -> Result<(RuleId, String), DomainError> {
  let rest = line.trim().trim_start_matches("##").trim();
  let Some((id_raw, title)) = rest.split_once(' ') else {
    return Err(DomainError::Parse(format!("heading missing title: {line}")));
  };
  let id_raw = id_raw.trim_end_matches('.').trim();
  if !is_valid_section_id(id_raw) {
    return Err(DomainError::InvalidRuleId(format!(
      "section id must be an integer or namespaced section like WASM-1: {id_raw}"
    )));
  }
  let title = title.trim();
  if title.is_empty() {
    return Err(DomainError::Parse(format!("heading missing title: {line}")));
  }
  Ok((RuleId::new(id_raw)?, title.to_string()))
}

/// A section id is a plain integer (`5`) or `NS-<n>` (`WASM-1`); dotted
/// sub-numbers are not allowed at section level.
fn is_valid_section_id(id: &str) -> bool {
  if id.chars().all(|c| c.is_ascii_digit()) {
    return true;
  }
  if let Some((ns, num)) = id.split_once('-') {
    return !ns.is_empty()
      && ns.chars().all(|c| c.is_ascii_alphabetic())
      && !num.is_empty()
      && num.chars().all(|c| c.is_ascii_digit());
  }
  false
}

/// Split a rule heading `### <n>.<m> <Title>` into `(id, title)`.
fn parse_rule_heading(line: &str) -> Result<(RuleId, String), DomainError> {
  let rest = line.trim().trim_start_matches("###").trim();
  let Some((id_raw, title)) = rest.split_once(' ') else {
    return Err(DomainError::Parse(format!("heading missing title: {line}")));
  };
  let title = title.trim();
  if title.is_empty() {
    return Err(DomainError::Parse(format!("heading missing title: {line}")));
  }
  Ok((RuleId::new(id_raw.trim())?, title.to_string()))
}

/// Collapse a raw body into a stable string: strip leading/trailing blank
/// lines and per-line trailing whitespace, preserving interior blank lines.
fn clean_body(lines: &[String]) -> String {
  let cleaned: Vec<&str> = lines.iter().map(|l| l.trim_end()).collect();
  let start = cleaned
    .iter()
    .position(|l| !l.is_empty())
    .unwrap_or(cleaned.len());
  let end = cleaned
    .iter()
    .rposition(|l| !l.is_empty())
    .map_or(start, |i| i + 1);
  cleaned[start..end].join("\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = r#"# AGENTS-RUST.md

Intro text that must be ignored.

---

## 1. Agent Behavior

### 1.1 Think Before Acting

- First bullet.
- Second bullet.

### 1.2 Simplicity

- Do the minimum.

---

## 2. User Interaction

- Ask one question at a time.
- Confirm once.

```text
## 3. This is inside a fence, not a section
### 3.9 Also not a rule
```

---

## 3. Checklist

- Run the checks.

[OVERRIDE §1.1] Use a different framing.
"#;

  #[test]
  fn parses_sections_subrules_and_overrides() {
    let rs = parse_agents_md(SAMPLE, "0.1.0").unwrap();
    assert_eq!(rs.version, "0.1.0");
    assert_eq!(rs.rules.len(), 4);
    assert_eq!(rs.overrides.len(), 1);

    assert_eq!(rs.rules[0].id.as_str(), "1.1");
    assert_eq!(rs.rules[0].section, "1");
    assert_eq!(rs.rules[0].title, "Think Before Acting");
    assert_eq!(rs.rules[0].severity, Severity::Mandatory);
    assert!(rs.rules[0].body.contains("First bullet."));

    assert_eq!(rs.rules[1].id.as_str(), "1.2");

    // Section without ### sub-headings becomes a single rule.
    assert_eq!(rs.rules[2].id.as_str(), "2");
    assert!(rs.rules[2].body.contains("one question"));

    assert_eq!(rs.rules[3].id.as_str(), "3");
    assert!(rs.rules[3].body.contains("Run the checks."));

    assert_eq!(rs.overrides[0].target_rule_id.as_str(), "1.1");
    assert_eq!(rs.overrides[0].reason, "Use a different framing.");
  }

  #[test]
  fn ignores_heading_lookalikes_inside_code_fences() {
    let rs = parse_agents_md(SAMPLE, "0.1.0").unwrap();
    assert!(rs.get_rule(&RuleId::new("3.9").unwrap()).is_none());
    // The fenced "## 3." is body text of section 2, not a real section.
    let section_two = rs.get_rule(&RuleId::new("2").unwrap()).unwrap();
    assert!(section_two.body.contains("inside a fence"));
    assert!(section_two.body.contains("### 3.9"));
  }

  #[test]
  fn drops_horizontal_rules_and_preamble() {
    let rs = parse_agents_md(SAMPLE, "0.1.0").unwrap();
    for rule in &rs.rules {
      assert!(!rule.body.contains("---"));
      assert!(!rule.body.contains("Intro text"));
    }
  }

  #[test]
  fn override_missing_target_is_rejected() {
    let src = "## 1. A\n\n- body\n\n[OVERRIDE §9.9] No such rule.\n";
    let err = parse_agents_md(src, "0.1.0").unwrap_err();
    assert!(matches!(err, DomainError::OverrideTargetNotFound(_)));
  }

  #[test]
  fn malformed_override_is_rejected() {
    let src = "## 1. A\n\n- body\n\n[OVERRIDE §] no id.\n";
    assert!(parse_agents_md(src, "0.1.0").is_err());
  }

  #[test]
  fn empty_input_is_rejected() {
    assert!(matches!(
      parse_agents_md("", "0.1.0").unwrap_err(),
      DomainError::EmptyRuleSet
    ));
  }

  #[test]
  fn dotted_section_heading_is_rejected() {
    let src = "## 5.2 Extra dot\n\n- body\n";
    assert!(matches!(
      parse_agents_md(src, "0.1.0").unwrap_err(),
      DomainError::InvalidRuleId(_)
    ));
  }

  #[test]
  fn duplicate_rule_ids_are_rejected() {
    let src = "### 1.1 One\n\n- a\n\n### 1.1 Two\n\n- b\n";
    assert!(matches!(
      parse_agents_md(src, "0.1.0").unwrap_err(),
      DomainError::DuplicateRuleId(_)
    ));
  }

  #[test]
  fn heading_missing_title_is_rejected() {
    assert!(matches!(
      parse_agents_md("## 1\n\n- body\n", "0.1.0").unwrap_err(),
      DomainError::Parse(_)
    ));
  }

  #[test]
  fn body_keeps_interior_blank_lines_and_trims_edges() {
    let src = "### 1.1 A\n\n  leading spaces\n\n    indented code\n\ntrailing   \n";
    let rs = parse_agents_md(src, "0.1.0").unwrap();
    let body = &rs.rules[0].body;
    assert!(!body.starts_with('\n'));
    assert!(!body.ends_with('\n'));
    assert!(body.contains("indented code"));
    assert!(body.contains("\n\n"));
    assert!(!body.contains("trailing   "));
  }

  #[test]
  fn records_section_headings() {
    let rs = parse_agents_md(SAMPLE, "0.1.0").unwrap();
    assert_eq!(rs.sections.len(), 3);
    assert_eq!(rs.sections[0].id.as_str(), "1");
    assert_eq!(rs.sections[0].title, "Agent Behavior");
    assert_eq!(rs.sections[2].id.as_str(), "3");
    assert_eq!(rs.sections[2].title, "Checklist");
    // Section 3 has a section-level rule sharing its id.
    assert!(rs.get_section(&RuleId::new("3").unwrap()).is_some());
  }

  #[test]
  fn parses_namespaced_fragment_sections() {
    let src = "## WASM-1. WebAssembly Targets\n\n### WASM-1.1 Memory\n\n- Keep it linear.\n\n## WASM-1.2 Sub\n\n- Not a section.\n";
    let err = parse_agents_md(src, "0.1.0").unwrap_err();
    assert!(matches!(err, DomainError::InvalidRuleId(_)));
  }

  #[test]
  fn parses_namespaced_fragment_ok() {
    let src = "## WASM-1. WebAssembly Targets\n\n### WASM-1.1 Memory\n\n- Keep it linear.\n\n### WASM-1.2 Async\n\n- Use wasm-bindgen.\n";
    let rs = parse_agents_md(src, "0.1.0").unwrap();
    assert_eq!(rs.rules.len(), 2);
    assert_eq!(rs.rules[0].id.as_str(), "WASM-1.1");
    assert_eq!(rs.rules[0].section, "WASM-1");
    assert_eq!(rs.rules[1].id.as_str(), "WASM-1.2");
    assert_eq!(rs.sections[0].id.as_str(), "WASM-1");
    assert_eq!(rs.sections[0].title, "WebAssembly Targets");
  }

  #[test]
  fn namespaced_override_targets_work() {
    let src = "## WASM-1. WebAssembly Targets\n\n### WASM-1.1 Memory\n\n- body\n\n[OVERRIDE §WASM-1.1] Use a custom allocator.\n";
    let rs = parse_agents_md(src, "0.1.0").unwrap();
    assert_eq!(rs.overrides.len(), 1);
    assert_eq!(rs.overrides[0].target_rule_id.as_str(), "WASM-1.1");
  }

  #[test]
  fn validate_collects_every_issue_with_line_numbers() {
    let src = "\
## 1. A

- body

### 1.1 Dup

- a

### 1.1 Dup

- b

[OVERRIDE §9.9] Orphan target.

[OVERRIDE §1.1] Fine.

[OVERRIDE §1.1] Duplicate target.

## 2. B

- c
";
    let report = validate_agents_md(src);
    assert_eq!(report.rule_count, 3);
    let kinds: Vec<ValidationIssueKind> = report.issues.iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&ValidationIssueKind::DuplicateRuleId));
    assert!(kinds.contains(&ValidationIssueKind::OrphanOverride));
    assert!(kinds.contains(&ValidationIssueKind::DuplicateOverride));
    for issue in &report.issues {
      assert!(issue.line >= 1);
      assert!(!issue.message.is_empty());
    }
  }

  #[test]
  fn validate_clean_document_has_no_issues() {
    let report = validate_agents_md(SAMPLE);
    assert_eq!(report.issue_count, 0);
    assert_eq!(report.rule_count, 4);
  }

  #[test]
  fn validate_empty_document_reports_empty_ruleset() {
    let report = validate_agents_md("");
    assert_eq!(report.rule_count, 0);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].kind, ValidationIssueKind::EmptyRuleSet);
  }

  #[test]
  fn validate_reports_malformed_override_line() {
    let src = "## 1. A\n\n- body\n\n[OVERRIDE §] broken\n";
    let report = validate_agents_md(src);
    assert!(
      report
        .issues
        .iter()
        .any(|i| i.kind == ValidationIssueKind::MalformedOverride)
    );
  }

  #[test]
  fn validation_report_serializes() {
    let report = validate_agents_md("### 1.1 A\n\n- body\n");
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"rule_count\""));
    assert!(json.contains("\"issues\""));
  }
}
