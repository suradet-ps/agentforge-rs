//! Benchmark harness for the ruleset build pipeline (`cargo bench`).
//!
//! These are in-process measurements of the composition work the CLI does
//! on every install; they are a regression guard, not a claim about the
//! wall-clock install time (see `scripts/perf-check.sh` for the binary
//! budgets).

use agentforge_builder::{
  BuildConfig, CORE_TEMPLATE, GENERATED_AT, RULESET_VERSION, TEMPLATES, build,
};
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_core_only(c: &mut Criterion) {
  let cfg = BuildConfig {
    core_template: CORE_TEMPLATE,
    fragments: &[],
    version: RULESET_VERSION,
    generated_at: GENERATED_AT,
  };
  c.bench_function("build/core-only", |b| b.iter(|| build(black_box(&cfg))));
}

fn bench_all_templates(c: &mut Criterion) {
  let frags: Vec<(&str, &str)> = TEMPLATES.iter().map(|t| (t.name, t.markdown)).collect();
  let cfg = BuildConfig {
    core_template: CORE_TEMPLATE,
    fragments: &frags,
    version: RULESET_VERSION,
    generated_at: GENERATED_AT,
  };
  c.bench_function("build/all-templates", |b| b.iter(|| build(black_box(&cfg))));
}

fn bench_parse_core(c: &mut Criterion) {
  c.bench_function("parse/core-template", |b| {
    b.iter(|| agentforge_domain::parse_agents_md(black_box(CORE_TEMPLATE), RULESET_VERSION))
  });
}

criterion_group!(
  benches,
  bench_core_only,
  bench_all_templates,
  bench_parse_core
);
criterion_main!(benches);
