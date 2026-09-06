//! Deterministic `RuleSet` → markdown rendering.
//!
//! The inverse of `agentforge_domain::parse_agents_md`. Rendering is
//! byte-for-byte reproducible for the same input: section order is document
//! order and no timestamps are emitted.

use agentforge_domain::rule::RuleSet;
use agentforge_domain::rule_id::RuleId;

/// Render a `RuleSet` back into `AGENTS-RUST.md` format.
///
/// Sections are emitted in document order with their recorded titles;
/// section-level rules (id == section id) appear directly under the `##`
/// heading, sub-rules under `###` headings, and overrides as
/// `[OVERRIDE §X]` lines at the end.
pub fn render_agents_md(rs: &RuleSet) -> String {
  let mut out = String::new();
  out.push_str("# AGENTS-RUST.md\n\n");
  out.push_str(&format!("Ruleset version: `{}`\n\n", rs.version));

  let mut current_section: Option<&str> = None;
  for rule in &rs.rules {
    if current_section != Some(rule.section.as_str()) {
      current_section = Some(rule.section.as_str());
      out.push_str(&section_heading(rs, rule.section.as_str()));
    }

    if rule.id.as_str() == rule.section {
      out.push_str(rule.body.trim_end());
      out.push_str("\n\n");
    } else {
      out.push_str(&format!("### {} {}\n\n", rule.id, rule.title));
      out.push_str(rule.body.trim_end());
      out.push_str("\n\n");
    }
  }

  for ovr in &rs.overrides {
    out.push_str(&format!(
      "[OVERRIDE §{}] {}\n",
      ovr.target_rule_id, ovr.reason
    ));
  }

  out
}

/// Render a `## <id>. <Title>` heading, falling back to the section-level
/// rule's title (and then to a bare `## <id>`) when no recorded title
/// exists.
fn section_heading(rs: &RuleSet, section: &str) -> String {
  let title = RuleId::new(section)
    .ok()
    .and_then(|id| rs.get_section(&id))
    .map(|s| s.title.clone())
    .or_else(|| {
      rs.rules
        .iter()
        .find(|r| r.section == section && r.id.as_str() == section)
        .map(|r| r.title.clone())
    });

  match title {
    Some(t) if !t.is_empty() => format!("## {section}. {t}\n\n"),
    _ => format!("## {section}\n\n"),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use agentforge_domain::r#override::Override;
  use agentforge_domain::rule::{Rule, RuleSet, Section, Severity};
  use agentforge_domain::rule_id::RuleId;

  fn sample_ruleset() -> RuleSet {
    let mut rs = RuleSet::new("1.0.0".into());
    rs.add_section(Section {
      id: RuleId::new("5").unwrap(),
      title: "Rust Idioms".into(),
    })
    .unwrap();
    rs.add_section(Section {
      id: RuleId::new("2").unwrap(),
      title: "Interaction".into(),
    })
    .unwrap();
    rs.add_rule(Rule::new(
      RuleId::new("5.2").unwrap(),
      "5".into(),
      "Error Handling".into(),
      "- Use thiserror.".into(),
      Severity::Recommended,
    ))
    .unwrap();
    rs.add_rule(Rule::new(
      RuleId::new("2").unwrap(),
      "2".into(),
      "Interaction".into(),
      "- Ask one question.".into(),
      Severity::Mandatory,
    ))
    .unwrap();
    rs.add_override(Override::new(
      RuleId::new("5.2").unwrap(),
      "Use anyhow.".into(),
    ))
    .unwrap();
    rs
  }

  #[test]
  fn renders_sections_subrules_and_overrides() {
    let out = render_agents_md(&sample_ruleset());
    assert!(out.starts_with("# AGENTS-RUST.md\n\nRuleset version: `1.0.0`\n\n"));
    assert!(out.contains("## 5. Rust Idioms\n\n### 5.2 Error Handling\n\n- Use thiserror."));
    assert!(out.contains("## 2. Interaction\n\n- Ask one question."));
    assert!(out.contains("[OVERRIDE §5.2] Use anyhow."));
  }

  #[test]
  fn section_level_rule_uses_section_heading() {
    let out = render_agents_md(&sample_ruleset());
    let idx5 = out.find("## 5. Rust Idioms").unwrap();
    let idx2 = out.find("## 2. Interaction").unwrap();
    assert!(idx5 < idx2);
  }

  #[test]
  fn render_parse_round_trip_is_content_stable() {
    let rs = sample_ruleset();
    let md = render_agents_md(&rs);
    let reparsed = agentforge_domain::parse_agents_md(&md, "1.0.0").unwrap();
    assert_eq!(reparsed.rules.len(), rs.rules.len());
    assert_eq!(reparsed.overrides.len(), rs.overrides.len());
    assert_eq!(reparsed.sections.len(), rs.sections.len());
    for (a, b) in rs.rules.iter().zip(reparsed.rules.iter()) {
      assert_eq!(a.id, b.id);
      assert_eq!(a.title, b.title);
      assert_eq!(a.body, b.body);
    }
  }

  #[test]
  fn deterministic_output() {
    let a = render_agents_md(&sample_ruleset());
    let b = render_agents_md(&sample_ruleset());
    assert_eq!(a, b);
  }
}
