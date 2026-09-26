//! The ruleset bundle: the single distributable artifact of a ruleset.
//!
//! A bundle carries the verbatim core constitution, every domain fragment,
//! and the manifest of the fully composed ruleset in one deterministic JSON
//! document. `update-rules` downloads and verifies a bundle; the release
//! workflow publishes one per ruleset version. JSON keeps the artifact
//! inspectable, diffable, and dependency-free to parse, in contrast to a
//! compressed archive.

use std::collections::BTreeMap;

use agentforge_builder::{
  BuildConfig, BuildOutput, CORE_TEMPLATE, GENERATED_AT, RULESET_VERSION, TEMPLATES, build,
  validation_report,
};
use agentforge_domain::manifest::RuleManifest;
use serde::{Deserialize, Serialize};

/// The only bundle schema this CLI understands.
pub const BUNDLE_VERSION: u32 = 1;

/// A ruleset release: core, fragments, and the composed manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RulesetBundle {
  /// Schema version of the bundle format itself.
  pub bundle_version: u32,
  /// Semver of the ruleset, independent of the CLI version.
  pub ruleset_version: String,
  /// Fixed generation timestamp for reproducibility.
  pub generated_at: String,
  /// The verbatim core constitution.
  pub core: String,
  /// Fragment markdown by template name (sorted for determinism).
  pub fragments: BTreeMap<String, String>,
  /// Manifest of the fully composed ruleset (core + every fragment).
  pub manifest: RuleManifest,
}

/// Errors from decoding or checking a bundle.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
  #[error("malformed bundle: {0}")]
  Malformed(String),

  #[error("unsupported bundle version {0}; update cargo-agentforge")]
  UnsupportedVersion(u32),

  #[error("bundle is inconsistent: its manifest does not match the composed rules")]
  Inconsistent,

  #[error("bundle does not contain the template `{0}` required by the installed ruleset")]
  MissingFragment(String),

  #[error("bundle contains unknown template `{0}`")]
  UnknownFragment(String),

  #[error("failed to compose the bundle: {0}")]
  Compose(String),

  #[error("bundle ruleset is not shippable: {0}")]
  InvalidRuleset(String),
}

/// Parse and version-check bundle bytes.
///
/// # Errors
///
/// Returns [`BundleError::Malformed`] for invalid JSON,
/// [`BundleError::UnsupportedVersion`] for a schema this CLI does not know,
/// and [`BundleError::Inconsistent`] when the manifest records a different
/// ruleset version than the bundle.
pub fn decode_bundle(bytes: &[u8]) -> Result<RulesetBundle, BundleError> {
  let bundle: RulesetBundle =
    serde_json::from_slice(bytes).map_err(|e| BundleError::Malformed(e.to_string()))?;

  if bundle.bundle_version != BUNDLE_VERSION {
    return Err(BundleError::UnsupportedVersion(bundle.bundle_version));
  }

  if bundle.manifest.ruleset_version != bundle.ruleset_version {
    return Err(BundleError::Inconsistent);
  }

  Ok(bundle)
}

/// Check a bundle against itself: composing the core plus every fragment in
/// the shipped template order must reproduce the bundled manifest, and the
/// composed ruleset must pass the validation pipeline with zero errors.
///
/// # Errors
///
/// Returns the matching [`BundleError`] when the bundle carries unknown
/// fragments, composes to a different ruleset, or fails validation.
pub fn verify_bundle(bundle: &RulesetBundle) -> Result<(), BundleError> {
  let fragments = all_fragments(bundle)?;
  let config = BuildConfig {
    core_template: &bundle.core,
    fragments: &fragments,
    version: &bundle.ruleset_version,
    generated_at: &bundle.generated_at,
  };

  let output = build(&config).map_err(|e| BundleError::Compose(e.to_string()))?;
  if !output.manifest.content_eq(&bundle.manifest) {
    return Err(BundleError::Inconsistent);
  }

  let report = validation_report(&config);
  if !report.errors.is_empty() {
    return Err(BundleError::InvalidRuleset(report.errors.join("; ")));
  }

  Ok(())
}

/// Compose the update target for the installed template selection.
///
/// # Errors
///
/// Returns [`BundleError::MissingFragment`] when a selected template is not
/// part of the bundle and [`BundleError::Compose`] when composition fails.
pub fn compose_target(
  bundle: &RulesetBundle,
  selection: &[&str],
) -> Result<BuildOutput, BundleError> {
  let mut fragments = Vec::with_capacity(selection.len());

  for name in selection {
    let markdown = bundle
      .fragments
      .get(*name)
      .ok_or_else(|| BundleError::MissingFragment((*name).to_string()))?;
    fragments.push((*name, markdown.as_str()));
  }

  build(&BuildConfig {
    core_template: &bundle.core,
    fragments: &fragments,
    version: &bundle.ruleset_version,
    generated_at: &bundle.generated_at,
  })
  .map_err(|e| BundleError::Compose(e.to_string()))
}

/// Every fragment in the bundle, in shipped template order.
fn all_fragments(bundle: &RulesetBundle) -> Result<Vec<(&str, &str)>, BundleError> {
  for name in bundle.fragments.keys() {
    if !TEMPLATES.iter().any(|t| t.name == name) {
      return Err(BundleError::UnknownFragment(name.clone()));
    }
  }

  Ok(
    TEMPLATES
      .iter()
      .filter_map(|t| bundle.fragments.get(t.name).map(|md| (t.name, md.as_str())))
      .collect(),
  )
}

/// Build the bundle that the release workflow publishes, from the templates
/// embedded in this binary.
///
/// # Errors
///
/// Returns [`BundleError::Compose`] when the embedded ruleset fails to build
/// (a release blocker), never a panic.
pub fn shipped_bundle() -> Result<RulesetBundle, BundleError> {
  let fragments: BTreeMap<String, String> = TEMPLATES
    .iter()
    .map(|t| (t.name.to_string(), t.markdown.to_string()))
    .collect();
  let all: Vec<(&str, &str)> = TEMPLATES.iter().map(|t| (t.name, t.markdown)).collect();
  let output = build(&BuildConfig {
    core_template: CORE_TEMPLATE,
    fragments: &all,
    version: RULESET_VERSION,
    generated_at: GENERATED_AT,
  })
  .map_err(|e| BundleError::Compose(e.to_string()))?;

  Ok(RulesetBundle {
    bundle_version: BUNDLE_VERSION,
    ruleset_version: RULESET_VERSION.to_string(),
    generated_at: GENERATED_AT.to_string(),
    core: CORE_TEMPLATE.to_string(),
    fragments,
    manifest: output.manifest,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn shipped_bundle_verifies() {
    verify_bundle(&shipped_bundle().unwrap()).unwrap();
  }

  #[test]
  fn shipped_bundle_round_trips_through_json() {
    let bundle = shipped_bundle().unwrap();
    let bytes = serde_json::to_vec(&bundle).unwrap();
    assert_eq!(decode_bundle(&bytes).unwrap(), bundle);
  }

  #[test]
  fn shipped_bundle_is_deterministic() {
    let a = serde_json::to_vec(&shipped_bundle().unwrap()).unwrap();
    let b = serde_json::to_vec(&shipped_bundle().unwrap()).unwrap();
    assert_eq!(a, b);
  }

  #[test]
  fn tampered_core_is_rejected() {
    let mut bundle = shipped_bundle().unwrap();
    bundle.core.push_str("\n## 99. Tampered\n\n- Extra rule.\n");
    assert!(matches!(
      verify_bundle(&bundle),
      Err(BundleError::Inconsistent)
    ));
  }

  #[test]
  fn unknown_fragment_is_rejected() {
    let mut bundle = shipped_bundle().unwrap();
    bundle
      .fragments
      .insert("nope".into(), "## NOPE-1. Not shipped\n".into());
    assert!(matches!(
      verify_bundle(&bundle),
      Err(BundleError::UnknownFragment(_))
    ));
  }

  #[test]
  fn unsupported_bundle_version_is_rejected() {
    let mut bundle = shipped_bundle().unwrap();
    bundle.bundle_version = 99;
    let bytes = serde_json::to_vec(&bundle).unwrap();
    assert!(matches!(
      decode_bundle(&bytes),
      Err(BundleError::UnsupportedVersion(99))
    ));
  }

  #[test]
  fn manifest_version_mismatch_is_rejected() {
    let mut bundle = shipped_bundle().unwrap();
    bundle.manifest.ruleset_version = "9.9.9".into();
    let bytes = serde_json::to_vec(&bundle).unwrap();
    assert!(matches!(
      decode_bundle(&bytes),
      Err(BundleError::Inconsistent)
    ));
  }

  #[test]
  fn garbage_is_rejected() {
    assert!(matches!(
      decode_bundle(b"not json"),
      Err(BundleError::Malformed(_))
    ));
  }

  #[test]
  fn compose_target_uses_only_the_selection() {
    let output = compose_target(&shipped_bundle().unwrap(), &["wasm"]).unwrap();
    assert!(output.markdown.contains("## WASM-1."));
    assert!(!output.markdown.contains("## TAURI-1."));
  }

  #[test]
  fn compose_target_rejects_missing_fragment() {
    let mut bundle = shipped_bundle().unwrap();
    bundle.fragments.remove("wasm");
    assert!(matches!(
      compose_target(&bundle, &["wasm"]),
      Err(BundleError::MissingFragment(_))
    ));
  }
}
