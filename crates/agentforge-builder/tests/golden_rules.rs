//! Golden-rule gate: the shipped ruleset must never drop or weaken a golden
//! rule. Runs under `cargo test --workspace`.
//!
//! `golden_rules.json` lists the fixed invariants (e.g. §3 mandates
//! `clippy -D warnings`, §5.4 mandates `// SAFETY:` comments). If a future
//! change removes or weakens any of them, this gate fails the build.

use agentforge_builder::{
  BuildConfig, CORE_TEMPLATE, GoldenRule, RULESET_VERSION, TEMPLATES, build, check_golden_rules,
};

const GOLDEN_JSON: &str = include_str!("golden_rules.json");

fn golden() -> Vec<GoldenRule> {
  serde_json::from_str(GOLDEN_JSON).unwrap()
}

fn parse_core() -> agentforge_domain::RuleSet {
  agentforge_domain::parse_agents_md(CORE_TEMPLATE, RULESET_VERSION).unwrap()
}

#[test]
fn core_only_ruleset_preserves_golden_rules() {
  let violations = check_golden_rules(&parse_core(), &golden());
  assert!(
    violations.is_empty(),
    "core ruleset downgraded a golden rule: {violations:?}"
  );
}

#[test]
fn composed_ruleset_preserves_golden_rules() {
  let frags: Vec<(&str, &str)> = TEMPLATES.iter().map(|t| (t.name, t.markdown)).collect();
  let out = build(&BuildConfig {
    core_template: CORE_TEMPLATE,
    fragments: &frags,
    version: RULESET_VERSION,
    generated_at: "2026-01-01T00:00:00Z",
  })
  .unwrap();
  let violations = check_golden_rules(&out.ruleset, &golden());
  assert!(
    violations.is_empty(),
    "composed ruleset downgraded a golden rule: {violations:?}"
  );
}

#[test]
fn removing_a_golden_rule_is_detected() {
  let mut downgraded = parse_core();
  downgraded.rules.retain(|r| r.id.as_str() != "3");
  let violations = check_golden_rules(&downgraded, &golden());
  assert!(
    violations.iter().any(|v| v.contains("§3")),
    "expected a §3 violation, got: {violations:?}"
  );
}

#[test]
fn weakening_a_golden_rule_is_detected() {
  let mut weakened = parse_core();
  for rule in &mut weakened.rules {
    if rule.id.as_str() == "5.2" {
      rule.body = "- Use anyhow instead.".into();
    }
  }
  let violations = check_golden_rules(&weakened, &golden());
  assert!(
    violations.iter().any(|v| v.contains("§5.2")),
    "expected a §5.2 violation, got: {violations:?}"
  );
}

#[test]
fn overrides_all_resolve_in_shipped_rulesets() {
  for source in [CORE_TEMPLATE] {
    let rs = agentforge_domain::parse_agents_md(source, RULESET_VERSION).unwrap();
    for ovr in &rs.overrides {
      assert!(
        rs.get_rule(&ovr.target_rule_id).is_some(),
        "override {} targets a missing rule",
        ovr
      );
    }
  }
}
