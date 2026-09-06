use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExitCode {
  Installed,
  Upgraded,
  Skipped,
  Conflict,
  DryRun,
  InputError,
  InternalError,
  Stale,
  NotInstalled,
  HasDiff,
}

impl ExitCode {
  pub fn as_i32(self) -> i32 {
    match self {
      Self::Installed => 0,
      Self::Upgraded => 0,
      Self::Skipped => 0,
      Self::Conflict => 3,
      Self::DryRun => 0,
      Self::InputError => 2,
      Self::InternalError => 1,
      Self::Stale => 4,
      Self::NotInstalled => 5,
      Self::HasDiff => 6,
    }
  }
}

impl fmt::Display for ExitCode {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Installed => write!(f, "installed"),
      Self::Upgraded => write!(f, "upgraded"),
      Self::Skipped => write!(f, "skipped (already up-to-date)"),
      Self::Conflict => write!(f, "conflict (local edits detected, use --force)"),
      Self::DryRun => write!(f, "dry-run (no changes written)"),
      Self::InputError => write!(f, "input error"),
      Self::InternalError => write!(f, "internal error"),
      Self::Stale => write!(
        f,
        "stale (installed ruleset is older than the bundled baseline)"
      ),
      Self::NotInstalled => write!(f, "not installed (no manifest found)"),
      Self::HasDiff => write!(f, "diff (installed ruleset differs from the target)"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn exit_code_values() {
    assert_eq!(ExitCode::Installed.as_i32(), 0);
    assert_eq!(ExitCode::Upgraded.as_i32(), 0);
    assert_eq!(ExitCode::Skipped.as_i32(), 0);
    assert_eq!(ExitCode::Conflict.as_i32(), 3);
    assert_eq!(ExitCode::DryRun.as_i32(), 0);
    assert_eq!(ExitCode::InputError.as_i32(), 2);
    assert_eq!(ExitCode::InternalError.as_i32(), 1);
    assert_eq!(ExitCode::Stale.as_i32(), 4);
    assert_eq!(ExitCode::NotInstalled.as_i32(), 5);
    assert_eq!(ExitCode::HasDiff.as_i32(), 6);
  }

  #[test]
  fn exit_code_display() {
    assert_eq!(ExitCode::Installed.to_string(), "installed");
    assert_eq!(
      ExitCode::Conflict.to_string(),
      "conflict (local edits detected, use --force)"
    );
    assert!(ExitCode::Stale.to_string().contains("stale"));
  }
}
