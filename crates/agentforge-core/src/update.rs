//! `update-rules` support: apply a fetched ruleset over an installed one
//! under the same "zero silent data loss" policy as `init`.
//!
//! The network itself stays outside this crate: a [`Fetcher`] delivers the
//! bytes of the target manifest and markdown, and [`update`] only decides
//! and writes. That keeps the core unit-testable without sockets and lets
//! the CLI own the TLS story.

use std::cmp::Ordering;

use agentforge_domain::manifest::RuleManifest;

use crate::check::compare_versions;
use crate::fs::InstallTarget;
use crate::install::{Config, CoreError, Outcome, write_ruleset};

/// Minimal transport abstraction for fetching remote rulesets.
///
/// Implementations perform the actual network I/O (in the CLI), with TLS
/// validation that is never disabled. Errors are human-readable transport
/// failures (DNS, TLS, HTTP status, timeout), not policy decisions.
pub trait Fetcher {
  /// Fetch the raw bytes at `url`.
  ///
  /// # Errors
  ///
  /// Returns a human-readable error when the transport fails.
  fn fetch(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// A verified SHA-256 digest (lowercase, 64 hex characters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum(String);

impl Checksum {
  /// Compute the digest of `bytes`.
  pub fn of(bytes: &[u8]) -> Self {
    Self(agentforge_domain::manifest::sha256_hex(bytes))
  }

  /// Parse a hex digest, normalizing to lowercase.
  ///
  /// # Errors
  ///
  /// Returns [`CoreError::MalformedChecksum`] when the string is not 64 hex
  /// characters.
  pub fn parse(hex: &str) -> Result<Self, CoreError> {
    let normalized = hex.trim().to_ascii_lowercase();
    if normalized.len() == 64 && normalized.bytes().all(|b| b.is_ascii_hexdigit()) {
      Ok(Self(normalized))
    } else {
      Err(CoreError::MalformedChecksum {
        value: hex.trim().to_string(),
      })
    }
  }

  /// Check `bytes` against this digest.
  ///
  /// # Errors
  ///
  /// Returns [`CoreError::ChecksumMismatch`] when the bytes do not match.
  pub fn verify(&self, subject: &str, bytes: &[u8]) -> Result<(), CoreError> {
    let actual = agentforge_domain::manifest::sha256_hex(bytes);
    if actual == self.0 {
      Ok(())
    } else {
      Err(CoreError::ChecksumMismatch {
        subject: subject.to_string(),
        expected: self.0.clone(),
        actual,
      })
    }
  }

  /// The lowercase hex digest.
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// Apply the target ruleset (`config.manifest` and `config.agents_md`) over
/// the one installed in `fs`.
///
/// Safety policy:
/// - refuses to downgrade (`installed > target`) unless `config.force`;
/// - detects local edits by comparing the actual checksums of the installed
///   `AGENTS-RUST.md` against the installed manifest baseline and reports
///   [`Outcome::Conflict`] unless `config.force`;
/// - identical content is [`Outcome::Skipped`];
/// - `config.dry_run` makes no writes.
///
/// # Errors
///
/// Returns [`CoreError::NotInstalled`] when the manifest or markdown is
/// missing, typed parse/serialize errors for corrupt input, and
/// [`CoreError::VersionDowngrade`] for a rejected downgrade.
pub fn update<F: InstallTarget>(fs: &F, config: &Config) -> Result<Outcome, CoreError> {
  let Some(manifest_json) = fs.read_file(&config.manifest_path) else {
    return Err(CoreError::NotInstalled {
      path: config.manifest_path.clone(),
    });
  };
  let installed: RuleManifest = serde_json::from_str(&manifest_json)
    .map_err(|e| CoreError::ManifestDeserialize(e.to_string()))?;

  let Some(installed_md) = fs.read_file(&config.agents_md_path) else {
    return Err(CoreError::NotInstalled {
      path: config.agents_md_path.clone(),
    });
  };
  let installed_rs = agentforge_domain::parse_agents_md(&installed_md, &installed.ruleset_version)
    .map_err(|source| CoreError::MarkdownParse {
      path: config.agents_md_path.clone(),
      source,
    })?;
  let actual =
    RuleManifest::from_rule_set(&installed_rs, &installed.generated_at).map_err(|source| {
      CoreError::ManifestBuild {
        path: config.agents_md_path.clone(),
        source,
      }
    })?;

  if compare_versions(&config.manifest.ruleset_version, &installed.ruleset_version)
    == Ordering::Less
    && !config.force
  {
    return Err(CoreError::VersionDowngrade {
      installed: installed.ruleset_version.clone(),
      incoming: config.manifest.ruleset_version.clone(),
    });
  }

  let edited_rules = local_edits(&installed, &actual);
  if !edited_rules.is_empty() && !config.force {
    return Ok(Outcome::Conflict { edited_rules });
  }

  if installed.content_eq(&config.manifest) {
    return Ok(Outcome::Skipped);
  }

  if config.dry_run {
    return Ok(Outcome::DryRun {
      would_install: true,
    });
  }

  write_ruleset(fs, config)?;
  Ok(Outcome::Upgraded)
}

/// Rule ids whose actual content differs from the installed baseline:
/// edited bodies, rules added by hand, and rules removed by hand.
fn local_edits(baseline: &RuleManifest, actual: &RuleManifest) -> Vec<String> {
  let mut edited = Vec::new();

  for rule in &actual.rules {
    match baseline.rules.iter().find(|b| b.id == rule.id) {
      Some(b) if b.body_checksum != rule.body_checksum => edited.push(rule.id.clone()),
      None => edited.push(rule.id.clone()),
      _ => {}
    }
  }

  for rule in &baseline.rules {
    if !actual.rules.iter().any(|a| a.id == rule.id) {
      edited.push(rule.id.clone());
    }
  }

  edited.sort();
  edited.dedup();
  edited
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fs::MockFs;

  const AT: &str = "2026-01-01T00:00:00Z";
  const V1: &str = "0.1.0";
  const V2: &str = "0.2.0";

  const BASELINE_MD: &str = "# AGENTS-RUST.md

## 0. Golden Rules

- These rules always apply.

## 5. Rust Idioms

### 5.2 Error Handling

- Use thiserror.
";

  const TARGET_MD: &str = "# AGENTS-RUST.md

## 0. Golden Rules

- These rules always apply.

## 5. Rust Idioms

### 5.2 Error Handling

- Use thiserror for errors.
";

  const EDITED_MD: &str = "# AGENTS-RUST.md

## 0. Golden Rules

- These rules always apply.

## 5. Rust Idioms

### 5.2 Error Handling

- Use anyhow.
";

  fn manifest_for(md: &str, version: &str) -> RuleManifest {
    let rs = agentforge_domain::parse_agents_md(md, version).unwrap();
    RuleManifest::from_rule_set(&rs, AT).unwrap()
  }

  fn target(md: &str, version: &str) -> Config {
    Config {
      manifest: manifest_for(md, version),
      agents_md: md.to_string(),
      agents_md_path: "/project/AGENTS-RUST.md".into(),
      manifest_path: "/project/.agentforge.json".into(),
      force: false,
      dry_run: false,
    }
  }

  /// An installation whose manifest records `baseline_md`, but whose actual
  /// `AGENTS-RUST.md` may have been edited to `actual_md`.
  fn installed_fs(baseline_md: &str, actual_md: &str, version: &str) -> MockFs {
    let baseline = manifest_for(baseline_md, version);
    MockFs::new()
      .with_file("/project/AGENTS-RUST.md", actual_md)
      .with_file(
        "/project/.agentforge.json",
        serde_json::to_string(&baseline).unwrap(),
      )
  }

  fn pristine_fs(md: &str, version: &str) -> MockFs {
    installed_fs(md, md, version)
  }

  #[test]
  fn up_to_date_is_skipped() {
    let fs = pristine_fs(BASELINE_MD, V1);
    let outcome = update(&fs, &target(BASELINE_MD, V1)).unwrap();
    assert_eq!(outcome, Outcome::Skipped);
  }

  #[test]
  fn upgrade_writes_the_target_ruleset() {
    let fs = pristine_fs(BASELINE_MD, V1);
    let config = target(TARGET_MD, V2);
    let outcome = update(&fs, &config).unwrap();
    assert_eq!(outcome, Outcome::Upgraded);
    assert_eq!(
      fs.read_file(&config.agents_md_path).as_deref(),
      Some(TARGET_MD)
    );
    let written: RuleManifest =
      serde_json::from_str(&fs.read_file(&config.manifest_path).unwrap()).unwrap();
    assert_eq!(written.ruleset_version, V2);
  }

  #[test]
  fn local_edit_conflicts_without_writes() {
    let fs = installed_fs(BASELINE_MD, EDITED_MD, V1);
    let config = target(TARGET_MD, V2);
    let outcome = update(&fs, &config).unwrap();
    assert_eq!(
      outcome,
      Outcome::Conflict {
        edited_rules: vec!["5.2".to_string()]
      }
    );
    assert_eq!(
      fs.read_file(&config.agents_md_path).as_deref(),
      Some(EDITED_MD),
      "a conflict must never touch the installed file"
    );
  }

  #[test]
  fn force_overwrites_local_edits() {
    let fs = installed_fs(BASELINE_MD, EDITED_MD, V1);
    let mut config = target(TARGET_MD, V2);
    config.force = true;
    let outcome = update(&fs, &config).unwrap();
    assert_eq!(outcome, Outcome::Upgraded);
    assert_eq!(
      fs.read_file(&config.agents_md_path).as_deref(),
      Some(TARGET_MD)
    );
  }

  #[test]
  fn locally_added_rule_conflicts() {
    let added_md = format!("{BASELINE_MD}\n## 6. Local\n\n### 6.1 Local Rule\n\n- Keep me.\n");
    let fs = installed_fs(BASELINE_MD, &added_md, V1);
    let outcome = update(&fs, &target(TARGET_MD, V2)).unwrap();
    assert_eq!(
      outcome,
      Outcome::Conflict {
        edited_rules: vec!["6.1".to_string()]
      }
    );
  }

  #[test]
  fn locally_removed_rule_conflicts() {
    let removed_md = "# AGENTS-RUST.md\n\n## 0. Golden Rules\n\n- These rules always apply.\n";
    let fs = installed_fs(BASELINE_MD, removed_md, V1);
    let outcome = update(&fs, &target(TARGET_MD, V2)).unwrap();
    assert_eq!(
      outcome,
      Outcome::Conflict {
        edited_rules: vec!["5.2".to_string()]
      }
    );
  }

  #[test]
  fn downgrade_is_rejected() {
    let fs = pristine_fs(TARGET_MD, V2);
    let err = update(&fs, &target(BASELINE_MD, V1)).unwrap_err();
    assert!(matches!(err, CoreError::VersionDowngrade { .. }));
  }

  #[test]
  fn force_allows_downgrade() {
    let fs = pristine_fs(TARGET_MD, V2);
    let mut config = target(BASELINE_MD, V1);
    config.force = true;
    assert_eq!(update(&fs, &config).unwrap(), Outcome::Upgraded);
  }

  #[test]
  fn dry_run_makes_no_writes() {
    let fs = pristine_fs(BASELINE_MD, V1);
    let mut config = target(TARGET_MD, V2);
    config.dry_run = true;
    let outcome = update(&fs, &config).unwrap();
    assert_eq!(
      outcome,
      Outcome::DryRun {
        would_install: true
      }
    );
    assert_eq!(
      fs.read_file(&config.agents_md_path).as_deref(),
      Some(BASELINE_MD)
    );
  }

  #[test]
  fn missing_installation_is_an_error() {
    let fs = MockFs::new();
    let err = update(&fs, &target(TARGET_MD, V2)).unwrap_err();
    assert!(matches!(err, CoreError::NotInstalled { .. }));
  }

  #[test]
  fn corrupt_manifest_is_an_error() {
    let fs = MockFs::new()
      .with_file("/project/AGENTS-RUST.md", BASELINE_MD)
      .with_file("/project/.agentforge.json", "not json");
    let err = update(&fs, &target(TARGET_MD, V2)).unwrap_err();
    assert!(matches!(err, CoreError::ManifestDeserialize(_)));
  }

  #[test]
  fn unparseable_markdown_is_an_error() {
    let fs = MockFs::new()
      .with_file("/project/AGENTS-RUST.md", "garbage with no rules")
      .with_file(
        "/project/.agentforge.json",
        serde_json::to_string(&manifest_for(BASELINE_MD, V1)).unwrap(),
      );
    let err = update(&fs, &target(TARGET_MD, V2)).unwrap_err();
    assert!(matches!(err, CoreError::MarkdownParse { .. }));
  }

  #[test]
  fn checksum_round_trips_and_detects_tampering() {
    let checksum = Checksum::of(b"payload");
    assert_eq!(checksum.as_str().len(), 64);
    checksum.verify("payload", b"payload").unwrap();
    let err = checksum.verify("payload", b"tampered").unwrap_err();
    assert!(matches!(err, CoreError::ChecksumMismatch { .. }));
  }

  #[test]
  fn checksum_parse_normalizes_and_rejects() {
    let digest = Checksum::of(b"abc");
    let upper = digest.as_str().to_ascii_uppercase();
    assert_eq!(Checksum::parse(&upper).unwrap(), digest);
    assert!(matches!(
      Checksum::parse("deadbeef"),
      Err(CoreError::MalformedChecksum { .. })
    ));
    assert!(matches!(
      Checksum::parse(&"z".repeat(64)),
      Err(CoreError::MalformedChecksum { .. })
    ));
  }

  struct StaticFetcher(Vec<u8>);

  impl Fetcher for StaticFetcher {
    fn fetch(&self, _url: &str) -> Result<Vec<u8>, String> {
      Ok(self.0.clone())
    }
  }

  #[test]
  fn fetcher_delivers_bytes() {
    let fetcher = StaticFetcher(b"ruleset bytes".to_vec());
    let bytes = fetcher.fetch("https://example.invalid/rules").unwrap();
    assert_eq!(bytes, b"ruleset bytes");
  }
}
