//! Reproducible-build verification (roadmap Phase 10).
//!
//! The same input must always produce byte-identical output: repeated builds
//! of the shipped ruleset may never differ in the rendered `AGENTS-RUST.md`
//! or in the serialized manifest. Any slice of iteration-order dependence
//! (hash maps), wall-clock time, or hand-rolled randomness breaks these
//! tests. The CI job `Reproducible build` runs the same check end to end
//! through the CLI (`scripts/repro-check.sh`).

use agentforge_builder::{BuildConfig, GENERATED_AT, RULESET_VERSION, build, templates};

fn build_selection(fragments: &[(&'static str, &'static str)]) -> (String, String) {
  let output = build(&BuildConfig {
    core_template: templates::CORE_TEMPLATE,
    fragments,
    version: RULESET_VERSION,
    generated_at: GENERATED_AT,
  })
  .expect("the shipped ruleset must build");
  let manifest_json =
    serde_json::to_string_pretty(&output.manifest).expect("the manifest must serialize");
  (output.markdown, manifest_json)
}

fn all_fragments() -> Vec<(&'static str, &'static str)> {
  templates::TEMPLATES
    .iter()
    .map(|t| (t.name, t.markdown))
    .collect()
}

#[test]
fn core_only_output_is_byte_identical_across_builds() {
  let first = build_selection(&[]);
  for run in 2..=3 {
    let next = build_selection(&[]);
    assert_eq!(first.0, next.0, "core-only markdown differs on build {run}");
    assert_eq!(first.1, next.1, "core-only manifest differs on build {run}");
  }
}

#[test]
fn composed_output_is_byte_identical_across_builds() {
  let fragments = all_fragments();
  let first = build_selection(&fragments);
  for run in 2..=3 {
    let next = build_selection(&fragments);
    assert_eq!(first.0, next.0, "composed markdown differs on build {run}");
    assert_eq!(first.1, next.1, "composed manifest differs on build {run}");
  }
}

#[test]
fn build_timestamps_are_pinned_by_input_only() {
  let (markdown, manifest) = build_selection(&all_fragments());
  let reparsed: agentforge_domain::RuleManifest =
    serde_json::from_str(&manifest).expect("the manifest must deserialize");
  assert_eq!(reparsed.generated_at, GENERATED_AT);
  assert!(
    !markdown.contains(GENERATED_AT),
    "the markdown must not embed build timestamps"
  );
}
