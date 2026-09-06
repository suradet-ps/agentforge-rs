//! Deterministic property test: for many generated valid documents,
//! `parse → render → parse` is stable and rendering is idempotent.

use agentforge_builder::render_agents_md;
use agentforge_domain::parse_agents_md;

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

/// Generate a valid `AGENTS-RUST.md`-shaped document with random sections,
/// sub-rules, and overrides that always target existing rules.
fn random_doc(rng: &mut Rng) -> String {
  let mut out = String::from("# AGENTS-RUST.md\n\nIntro.\n\n");
  let mut rule_ids: Vec<String> = Vec::new();
  let n_sections = 1 + rng.below(5);

  for s in 1..=n_sections {
    out.push_str(&format!("## {s}. Section {s}\n\n"));
    let n_sub = rng.below(4);
    if n_sub == 0 {
      out.push_str(&format!("- section body {s}\n\n"));
      rule_ids.push(format!("{s}"));
    } else {
      for m in 1..=n_sub {
        let id = format!("{s}.{m}");
        out.push_str(&format!(
          "### {id} Rule {id}\n\n- bullet {m} of {s}\n- second bullet\n\n"
        ));
        rule_ids.push(id);
      }
    }
  }

  let n_ovr = rng.below(3);
  let mut used: Vec<String> = Vec::new();
  for _ in 0..n_ovr {
    if rule_ids.is_empty() {
      break;
    }
    let idx = rng.below(rule_ids.len() as u64) as usize;
    let target = rule_ids[idx].clone();
    if used.contains(&target) {
      continue;
    }
    used.push(target.clone());
    out.push_str(&format!("[OVERRIDE §{target}] property override\n"));
  }

  out
}

#[test]
fn parse_render_round_trip_is_stable_and_render_is_idempotent() {
  let mut rng = Rng::new(0x5EED_2026);
  for iter in 0..500u64 {
    let doc = random_doc(&mut rng);
    let rs = parse_agents_md(&doc, "1.0.0").unwrap();

    let md1 = render_agents_md(&rs);
    let rs2 = parse_agents_md(&md1, "1.0.0").unwrap();

    assert_eq!(rs.rules.len(), rs2.rules.len(), "iter {iter}");
    assert_eq!(rs.sections.len(), rs2.sections.len(), "iter {iter}");
    assert_eq!(rs.overrides.len(), rs2.overrides.len(), "iter {iter}");
    for (a, b) in rs.rules.iter().zip(rs2.rules.iter()) {
      assert_eq!(a.id, b.id, "iter {iter}");
      assert_eq!(a.body, b.body, "iter {iter}");
    }

    // Rendering the reparsed ruleset must be byte-identical (idempotent).
    let md2 = render_agents_md(&rs2);
    assert_eq!(md1, md2, "render not idempotent at iter {iter}");
  }
}
