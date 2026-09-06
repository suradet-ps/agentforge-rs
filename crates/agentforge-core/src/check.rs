//! `check` support: report whether the installed ruleset matches the
//! bundled baseline.

use std::cmp::Ordering;

use agentforge_domain::manifest::RuleManifest;
use serde::Serialize;

use crate::exit::ExitCode;
use crate::fs::InstallTarget;
use crate::install::{Config, CoreError};

/// Result of comparing the installed ruleset against a target (bundled) one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CheckStatus {
  /// Installed ruleset is not older than the bundled baseline.
  UpToDate,
  /// Installed ruleset version is older than the bundled baseline.
  Stale { installed: String, bundled: String },
  /// No manifest is installed yet.
  NotInstalled,
}

impl CheckStatus {
  /// The process exit code associated with this status.
  pub fn exit_code(&self) -> ExitCode {
    match self {
      Self::UpToDate => ExitCode::Installed,
      Self::Stale { .. } => ExitCode::Stale,
      Self::NotInstalled => ExitCode::NotInstalled,
    }
  }
}

/// Compare the installed ruleset against `config.manifest` (the bundled
/// baseline) and report whether an update is needed.
///
/// # Errors
///
/// Returns `CoreError::ManifestDeserialize` when the installed manifest is
/// corrupt. A missing manifest is reported as
/// [`CheckStatus::NotInstalled`], not an error.
pub fn check_status<F: InstallTarget>(fs: &F, config: &Config) -> Result<CheckStatus, CoreError> {
  let Some(json) = fs.read_file(&config.manifest_path) else {
    return Ok(CheckStatus::NotInstalled);
  };

  let existing: RuleManifest =
    serde_json::from_str(&json).map_err(|e| CoreError::ManifestDeserialize(e.to_string()))?;

  if compare_versions(&existing.ruleset_version, &config.manifest.ruleset_version) == Ordering::Less
  {
    Ok(CheckStatus::Stale {
      installed: existing.ruleset_version,
      bundled: config.manifest.ruleset_version.clone(),
    })
  } else {
    Ok(CheckStatus::UpToDate)
  }
}

/// Compare two semver-ish version strings numerically, part by part
/// (`"0.1.0" < "1.0.0"`, `"1.2" < "1.2.1"`). Non-numeric segments are
/// ignored so partial versions compare predictably.
fn compare_versions(a: &str, b: &str) -> Ordering {
  let parts = |s: &str| -> Vec<u64> {
    s.split(|c: char| !c.is_ascii_digit())
      .filter(|p| !p.is_empty())
      .map(|p| p.parse::<u64>().unwrap_or(0))
      .collect()
  };

  let pa = parts(a);
  let pb = parts(b);

  for (x, y) in pa.iter().zip(pb.iter()) {
    let ord = x.cmp(y);
    if ord != Ordering::Equal {
      return ord;
    }
  }
  pa.len().cmp(&pb.len())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fs::MockFs;
  use agentforge_domain::rule::{Rule, RuleSet, Severity};
  use agentforge_domain::rule_id::RuleId;

  fn make_config(version: &str) -> Config {
    let mut rs = RuleSet::new(version.into());
    rs.add_rule(Rule::new(
      RuleId::new("0").unwrap(),
      "0".into(),
      "Golden Rules".into(),
      "Body".into(),
      Severity::Mandatory,
    ))
    .unwrap();
    Config {
      manifest: RuleManifest::from_rule_set(&rs, "2026-01-01T00:00:00Z").unwrap(),
      agents_md: "md".into(),
      agents_md_path: "/project/AGENTS-RUST.md".into(),
      manifest_path: "/project/.agentforge.json".into(),
      force: false,
      dry_run: false,
    }
  }

  #[test]
  fn up_to_date_when_versions_equal() {
    let config = make_config("1.0.0");
    let json = serde_json::to_string(&config.manifest).unwrap();
    let fs = MockFs::new().with_file("/project/.agentforge.json", json);
    assert_eq!(check_status(&fs, &config).unwrap(), CheckStatus::UpToDate);
  }

  #[test]
  fn up_to_date_when_installed_newer() {
    let installed = make_config("1.2.0");
    let json = serde_json::to_string(&installed.manifest).unwrap();
    let fs = MockFs::new().with_file("/project/.agentforge.json", json);

    let config = make_config("1.0.0");
    assert_eq!(check_status(&fs, &config).unwrap(), CheckStatus::UpToDate);
  }

  #[test]
  fn stale_when_installed_older() {
    let installed = make_config("0.9.0");
    let json = serde_json::to_string(&installed.manifest).unwrap();
    let fs = MockFs::new().with_file("/project/.agentforge.json", json);

    let config = make_config("1.0.0");
    let status = check_status(&fs, &config).unwrap();
    assert_eq!(
      status,
      CheckStatus::Stale {
        installed: "0.9.0".into(),
        bundled: "1.0.0".into()
      }
    );
    assert_eq!(status.exit_code(), ExitCode::Stale);
  }

  #[test]
  fn not_installed_when_manifest_missing() {
    let fs = MockFs::new();
    let config = make_config("1.0.0");
    assert_eq!(
      check_status(&fs, &config).unwrap(),
      CheckStatus::NotInstalled
    );
  }

  #[test]
  fn corrupt_manifest_is_an_error() {
    let fs = MockFs::new().with_file("/project/.agentforge.json", "not json");
    let config = make_config("1.0.0");
    assert!(check_status(&fs, &config).is_err());
  }

  #[test]
  fn version_comparison() {
    assert_eq!(compare_versions("0.1.0", "1.0.0"), Ordering::Less);
    assert_eq!(compare_versions("1.0.0", "0.9.9"), Ordering::Greater);
    assert_eq!(compare_versions("1.2", "1.2.1"), Ordering::Less);
    assert_eq!(compare_versions("2.0.0", "2.0.0"), Ordering::Equal);
  }
}
