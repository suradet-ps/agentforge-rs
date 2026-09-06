//! Deterministic property-style tests for the domain model: a seeded PRNG
//! drives many random inputs, so the invariants hold across a large input
//! space without pulling in a property-testing dependency.

use agentforge_domain::{
  Override, Rule, RuleId, RuleSet, Severity, parse_agents_md, validate_agents_md,
};

/// Tiny xorshift* PRNG for reproducible generation.
struct Rng(u64);

impl Rng {
  fn new(seed: u64) -> Self {
    Rng(seed.max(1))
  }

  fn next(&mut self) -> u64 {
    let mut x = self.0;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    self.0 = x;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
  }

  fn below(&mut self, n: u64) -> u64 {
    self.next() % n
  }
}

/// An alphabet heavy in markdown structure so random strings exercise the
/// parser's heading/fence/override detection paths.
const MARKDOWN_ALPHABET: &[char] = &[
  '#', '[', ']', 'O', 'V', 'E', 'R', 'I', 'D', '§', '`', '-', '.', ' ', '\n', '0', '1', '5', '9',
  'W', 'A', 'S', 'M', 'T', 'A', 'U', 'R', 'a', 'b', 'c', ':', '"', '<', '>', '&',
];

fn random_string(rng: &mut Rng, max_len: usize) -> String {
  let len = rng.below(max_len as u64) as usize;
  (0..len)
    .map(|_| MARKDOWN_ALPHABET[rng.below(MARKDOWN_ALPHABET.len() as u64) as usize])
    .collect()
}

/// The parser must never panic, on any input: it either parses to a
/// `RuleSet` or returns a typed error.
#[test]
fn malformed_inputs_error_but_never_panic() {
  let mut rng = Rng::new(0xBAD_F00D);
  for _ in 0..3000 {
    let input = random_string(&mut rng, 300);
    let _ = parse_agents_md(&input, "0.0.0");
    let _ = validate_agents_md(&input);
  }

  // Hand-picked hostile inputs.
  let nasty = [
    "",
    "```",
    "```\n## 1. X\n- body",
    "## 1. X\n- body\n```",
    "### 1.1",
    "## 1.",
    "## .1. X",
    "[OVERRIDE",
    "[OVERRIDE §]",
    "[OVERRIDE §5.2]",
    "[OVERRIDE §5.2.3.4.5] reason",
    "## WASM-1. X\n### WASM-1.1 Y\n- body",
    "### 5.2 Error\n- body\n### 5.2 Duplicate\n- body",
    "\u{0}\u{1}\u{2}",
    "## 99999999999999999999999999999. X\n- body",
  ];
  for input in nasty {
    let _ = parse_agents_md(input, "0.0.0");
    let _ = validate_agents_md(input);
  }
}

fn random_ruleset(rng: &mut Rng, version: &str) -> RuleSet {
  let mut rs = RuleSet::new(version.to_string());
  let count = 1 + rng.below(12);
  let mut used = std::collections::HashSet::new();
  for i in 0..count {
    let major = 1 + rng.below(20);
    let id = if rng.below(2) == 0 {
      major.to_string()
    } else {
      format!("{major}.{}", 1 + rng.below(9))
    };
    if !used.insert(id.clone()) {
      continue;
    }
    rs.add_rule(Rule::new(
      RuleId::new(&id).unwrap(),
      major.to_string(),
      format!("Rule {i}"),
      format!("- body {}", rng.next()),
      Severity::Mandatory,
    ))
    .unwrap();
  }
  rs
}

/// Every override added to a ruleset resolves to an existing rule, and
/// targeting a nonexistent rule is always rejected.
#[test]
fn overrides_resolve_and_orphans_are_rejected() {
  let mut rng = Rng::new(0x0F00_D00D);
  for _ in 0..1000 {
    let mut rs = random_ruleset(&mut rng, "1.0.0");

    // Pick an existing rule id — override must resolve.
    let idx = rng.below(rs.rules.len() as u64) as usize;
    let target = rs.rules[idx].id.clone();
    rs.add_override(Override::new(target.clone(), "why".into()))
      .unwrap();
    assert!(rs.get_rule(&target).is_some());
    assert_eq!(
      rs.get_override(&target).map(|o| o.target_rule_id.as_str()),
      Some(target.as_str())
    );

    // A random id must be rejected unless it happens to exist.
    let bogus = format!("999.{}", rng.below(100));
    if rs.get_rule(&RuleId::new(&bogus).unwrap()).is_none() {
      assert!(
        rs.add_override(Override::new(RuleId::new(&bogus).unwrap(), "no".into()))
          .is_err()
      );
    }
  }
}

/// Adding and then reverting an override leaves the pristine rule set
/// (rules and sections) completely unchanged.
#[test]
fn apply_then_revert_override_is_a_noop_on_rules() {
  let mut rng = Rng::new(0x4E0F_2026);
  for _ in 0..500 {
    let mut rs = random_ruleset(&mut rng, "1.0.0");
    let snapshot_rules = rs.rules.clone();
    let snapshot_sections = rs.sections.clone();

    let idx = rng.below(rs.rules.len() as u64) as usize;
    let target = rs.rules[idx].id.clone();
    rs.add_override(Override::new(target, "temporary".into()))
      .unwrap();

    // Revert: drop the override.
    rs.overrides.clear();

    assert_eq!(rs.rules, snapshot_rules);
    assert_eq!(rs.sections, snapshot_sections);
  }
}
