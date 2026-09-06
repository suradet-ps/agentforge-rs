//! Markdown → [`RuleSet`] parser for `AGENTS-RUST.md`.
//!
//! The constitution is authored as a markdown document, but every downstream
//! feature (versioning, diffing, safe updates, validation) needs a typed
//! model. This module turns the human-readable document into the domain
//! model without ever panicking: every malformed input produces a typed
//! [`DomainError`].

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
  Parser::new(version).parse(source, true)
}

/// Parse a domain fragment where `[OVERRIDE §X]` targets may reference rules
/// in the *composed* set (core + other fragments) rather than the fragment
/// alone. Override targets are therefore **not** validated here; the caller
/// (the builder) validates them after merging.
pub fn parse_agents_md_fragment(source: &str, version: &str) -> Result<RuleSet, DomainError> {
  Parser::new(version).parse(source, false)
}

struct Pending {
  id: RuleId,
  section: String,
  title: String,
  body: Vec<String>,
}

struct Parser<'a> {
  version: &'a str,
  rules: Vec<Rule>,
  overrides: Vec<Override>,
  sections: Vec<Section>,
  pending: Option<Pending>,
  current_section: Option<String>,
  in_code_fence: bool,
  validate_overrides: bool,
}

impl<'a> Parser<'a> {
  fn new(version: &'a str) -> Self {
    Self {
      version,
      rules: Vec::new(),
      overrides: Vec::new(),
      sections: Vec::new(),
      pending: None,
      current_section: None,
      in_code_fence: false,
      validate_overrides: true,
    }
  }

  fn parse(mut self, source: &str, validate_overrides: bool) -> Result<RuleSet, DomainError> {
    self.validate_overrides = validate_overrides;
    for line in source.lines() {
      let trimmed = line.trim();

      if trimmed.starts_with("```") {
        self.in_code_fence = !self.in_code_fence;
        if let Some(pending) = &mut self.pending {
          pending.body.push(line.to_string());
        }
        continue;
      }

      if self.in_code_fence {
        if let Some(pending) = &mut self.pending {
          pending.body.push(line.to_string());
        }
        continue;
      }

      if trimmed == "---" {
        continue;
      }

      if trimmed.starts_with("## ") {
        self.flush_pending()?;
        let (id, title) = parse_section_heading(trimmed)?;
        let id_str = id.as_str().to_string();
        self.sections.push(Section {
          id: id.clone(),
          title: title.clone(),
        });
        self.current_section = Some(id_str.clone());
        self.pending = Some(Pending {
          id,
          section: id_str,
          title,
          body: Vec::new(),
        });
        continue;
      }

      if trimmed.starts_with("### ") {
        self.flush_pending()?;
        let (id, title) = parse_rule_heading(trimmed)?;
        let section = self.current_section.clone().unwrap_or_else(|| {
          id.as_str()
            .split('.')
            .next()
            .unwrap_or_default()
            .to_string()
        });
        self.pending = Some(Pending {
          id,
          section,
          title,
          body: Vec::new(),
        });
        continue;
      }

      if trimmed.starts_with("[OVERRIDE") {
        let ovr = Override::parse_line(trimmed)?;
        self.overrides.push(ovr);
        continue;
      }

      if let Some(pending) = &mut self.pending {
        pending.body.push(line.to_string());
      }
    }

    self.flush_pending()?;

    if self.rules.is_empty() {
      return Err(DomainError::EmptyRuleSet);
    }

    let mut rs = RuleSet::new(self.version.to_string());
    for section in self.sections {
      rs.add_section(section)?;
    }
    for rule in self.rules {
      rs.add_rule(rule)?;
    }
    for ovr in self.overrides {
      if self.validate_overrides {
        rs.add_override(ovr)?;
      } else {
        rs.overrides.push(ovr);
      }
    }
    Ok(rs)
  }

  fn flush_pending(&mut self) -> Result<(), DomainError> {
    let Some(pending) = self.pending.take() else {
      return Ok(());
    };
    let body = clean_body(&pending.body);
    if body.is_empty() {
      return Ok(());
    }
    self.rules.push(Rule::new(
      pending.id,
      pending.section,
      pending.title,
      body,
      Severity::Mandatory,
    ));
    Ok(())
  }
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
}
