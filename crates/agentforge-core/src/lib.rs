mod check;
mod diff;
mod exit;
mod fs;
mod install;

pub use agentforge_domain;

pub use check::{CheckStatus, check_status};
pub use diff::{Change, DiffReport, RuleDiff, diff_manifests};
pub use exit::ExitCode;
pub use fs::{InstallTarget, RealFs};
pub use install::{Config, CoreError, Outcome, install};
