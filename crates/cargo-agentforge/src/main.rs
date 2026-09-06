//! `cargo agentforge` — install, check, and report the `AGENTS-RUST.md`
//! constitution for Rust projects.
//!
//! The binary follows the `cargo` subcommand convention: it is invoked as
//! `cargo agentforge <subcommand>`. With no subcommand it defaults to `init`.

use std::path::PathBuf;

use agentforge_core::{Config, CoreError, ExitCode, RealFs, check_status, install};
use agentforge_domain::RuleManifest;

use clap::{Args, Parser, Subcommand};

const AGENTS_FILE: &str = "AGENTS-RUST.md";
const MANIFEST_FILE: &str = ".agentforge.json";

const TEMPLATE: &str = include_str!("../templates/AGENTS-RUST.md");

/// Version of the bundled rule set, independent of the CLI version.
const RULESET_VERSION: &str = "0.1.0";

/// Fixed generation timestamp so repeated installs produce byte-identical
/// manifests (and therefore idempotent `skip` outcomes).
const GENERATED_AT: &str = "2026-01-01T00:00:00Z";

#[derive(Parser)]
#[command(
  name = "cargo-agentforge",
  version,
  about = "Install and maintain AGENTS-RUST.md in Rust projects"
)]
struct Cli {
  #[command(subcommand)]
  command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
  /// Install or upgrade the bundled constitution (default when no subcommand is given).
  Init(InitArgs),
  /// Report whether the installed ruleset is older than the bundled baseline.
  Check,
  /// Print the CLI version. Never touches the network or the filesystem.
  Version,
}

#[derive(Args, Default)]
struct InitArgs {
  /// Overwrite locally-edited rules instead of reporting a conflict.
  #[arg(long)]
  force: bool,
  /// Print what would change without writing anything.
  #[arg(long)]
  dry_run: bool,
}

fn main() {
  let cli = Cli::parse();

  let exit = match cli.command.unwrap_or(Command::Init(InitArgs::default())) {
    Command::Init(args) => run_init(&args),
    Command::Check => run_check(),
    Command::Version => {
      println!("cargo-agentforge {}", env!("CARGO_PKG_VERSION"));
      ExitCode::Installed
    }
  };

  std::process::exit(exit.as_i32());
}

fn run_init(args: &InitArgs) -> ExitCode {
  let manifest = match bundled_manifest() {
    Ok(m) => m,
    Err(e) => {
      eprintln!("internal error: bundled template failed to parse: {e}");
      return ExitCode::InternalError;
    }
  };

  let config = Config {
    manifest,
    agents_md: TEMPLATE.to_string(),
    agents_md_path: PathBuf::from(AGENTS_FILE),
    manifest_path: PathBuf::from(MANIFEST_FILE),
    force: args.force,
    dry_run: args.dry_run,
  };

  match install(&RealFs, &config) {
    Ok(outcome) => {
      print_outcome(&outcome, &config);
      outcome.exit_code()
    }
    Err(e) => print_error_and_exit(e),
  }
}

fn run_check() -> ExitCode {
  let manifest = match bundled_manifest() {
    Ok(m) => m,
    Err(e) => {
      eprintln!("internal error: bundled template failed to parse: {e}");
      return ExitCode::InternalError;
    }
  };

  let config = Config {
    manifest,
    agents_md: String::new(),
    agents_md_path: PathBuf::from(AGENTS_FILE),
    manifest_path: PathBuf::from(MANIFEST_FILE),
    force: false,
    dry_run: false,
  };

  match check_status(&RealFs, &config) {
    Ok(status) => {
      use agentforge_core::CheckStatus;
      match status {
        CheckStatus::UpToDate => {
          println!("✓ {} is up to date.", AGENTS_FILE);
          ExitCode::Installed
        }
        CheckStatus::Stale { installed, bundled } => {
          eprintln!(
            "✗ {} is stale: installed ruleset {installed}, bundled ruleset {bundled}.",
            AGENTS_FILE
          );
          ExitCode::Stale
        }
        CheckStatus::NotInstalled => {
          eprintln!(
            "✗ {MANIFEST_FILE} not found. Run `cargo agentforge init` to install the constitution."
          );
          ExitCode::NotInstalled
        }
      }
    }
    Err(e) => print_error_and_exit(e),
  }
}

/// Parse the embedded template into the manifest used for install/check.
fn bundled_manifest() -> Result<RuleManifest, agentforge_domain::DomainError> {
  let ruleset = agentforge_domain::parse_agents_md(TEMPLATE, RULESET_VERSION)?;
  RuleManifest::from_rule_set(&ruleset, GENERATED_AT)
}

fn print_outcome(outcome: &agentforge_core::Outcome, config: &Config) {
  use agentforge_core::Outcome;
  match outcome {
    Outcome::Installed => {
      println!(
        "✅ Installed {} (ruleset {})",
        AGENTS_FILE, config.manifest.ruleset_version
      );
    }
    Outcome::Upgraded => {
      println!(
        "✅ Upgraded {} to ruleset {}",
        AGENTS_FILE, config.manifest.ruleset_version
      );
    }
    Outcome::Skipped => {
      println!("✓ {} is already up to date.", AGENTS_FILE);
    }
    Outcome::Conflict { edited_rules } => {
      eprintln!(
        "✗ Conflict: {} has local edits on rules: {}",
        AGENTS_FILE,
        edited_rules.join(", ")
      );
      eprintln!("  Re-run with --force to overwrite them.");
    }
    Outcome::DryRun {
      would_install: true,
    } => {
      println!(
        "--dry-run: would install {} (ruleset {})",
        AGENTS_FILE, config.manifest.ruleset_version
      );
    }
    Outcome::DryRun {
      would_install: false,
    } => {
      println!("--dry-run: no changes needed.");
    }
  }
}

fn print_error_and_exit(e: CoreError) -> ExitCode {
  use agentforge_core::CoreError as E;
  eprintln!("error: {e}");
  match e {
    E::VersionMismatch { .. } => ExitCode::Conflict,
    E::ManifestRead { .. } | E::ManifestDeserialize(_) | E::WriteFailed { .. } => {
      ExitCode::InputError
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bundled_template_parses_all_sections() {
    let ruleset = agentforge_domain::parse_agents_md(TEMPLATE, RULESET_VERSION).unwrap();
    let ids: Vec<String> = ruleset.rules.iter().map(|r| r.id.to_string()).collect();
    assert_eq!(ruleset.rules.len(), 27);
    assert_eq!(ids[0], "0");
    assert_eq!(ids.last().map(String::as_str), Some("14"));
    assert!(ruleset.overrides.is_empty());
  }

  #[test]
  fn bundled_manifest_is_valid() {
    let manifest = bundled_manifest().unwrap();
    assert_eq!(manifest.ruleset_version, RULESET_VERSION);
    assert_eq!(manifest.rule_count, 27);
    assert!(manifest.rules.iter().all(|r| !r.body_checksum.is_empty()));
    assert!(manifest.overrides.is_empty());
  }
}
