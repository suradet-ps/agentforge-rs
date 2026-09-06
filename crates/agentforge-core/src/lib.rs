mod check;
mod exit;
mod fs;
mod install;

pub use agentforge_domain;

pub use check::{CheckStatus, check_status};
pub use exit::ExitCode;
pub use fs::InstallTarget;
pub use install::{Config, Outcome, install};
