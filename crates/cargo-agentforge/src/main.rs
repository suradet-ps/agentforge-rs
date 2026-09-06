//! `cargo agentforge` — install, check, and report the `AGENTS-RUST.md`
//! constitution for Rust projects.
//!
//! The binary follows the `cargo` subcommand convention: it is invoked as
//! `cargo agentforge <subcommand>`. With no subcommand it defaults to `init`.

use std::path::PathBuf;

use agentforge_builder::{CORE_TEMPLATE, GENERATED_AT, RULESET_VERSION};
use agentforge_core::{Config, CoreError, ExitCode, RealFs, check_status, install};

use clap::{Args, Parser, Subcommand};

const AGENTS_FILE: &str = "AGENTS-RUST.md";
const MANIFEST_FILE: &str = ".agentforge.json";

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
  /// List the available domain template fragments and their descriptions.
  Templates,
}

#[derive(Args, Default)]
struct InitArgs {
  /// Comma-separated domain templates to compose, e.g. `--template wasm,tauri`.
  #[arg(long, value_name = "TEMPLATES")]
  template: Option<String>,
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
    Command::Templates => run_templates(),
  };

  std::process::exit(exit.as_i32());
}

fn run_init(args: &InitArgs) -> ExitCode {
  let output = match compose(args.template.as_deref()) {
    Ok(o) => o,
    Err(e) => {
      eprintln!("error: {e}");
      return ExitCode::InputError;
    }
  };

  let config = Config {
    manifest: output.manifest,
    agents_md: output.markdown,
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
  let manifest = match compose(None) {
    Ok(o) => o.manifest,
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

/// Compose the bundled ruleset, optionally with domain fragments selected
/// via `--template wasm,tauri`. Core-only builds return the verbatim core
/// template; composed builds are re-rendered deterministically.
fn compose(selection: Option<&str>) -> Result<agentforge_builder::BuildOutput, String> {
  let fragments = match selection {
    Some(sel) => agentforge_builder::resolve_selection(sel)
      .map_err(|unknown| format!("unknown template: {unknown}"))?,
    None => Vec::new(),
  };

  agentforge_builder::build(&agentforge_builder::BuildConfig {
    core_template: CORE_TEMPLATE,
    fragments: &fragments,
    version: RULESET_VERSION,
    generated_at: GENERATED_AT,
  })
  .map_err(|e| format!("failed to compose ruleset: {e}"))
}

fn run_templates() -> ExitCode {
  println!(
    "Available domain templates (compose with `cargo agentforge init --template <name>`):\n"
  );
  for t in agentforge_builder::TEMPLATES {
    println!("  {:<10} {}", t.name, t.description);
  }
  ExitCode::Installed
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
    let ruleset = agentforge_domain::parse_agents_md(CORE_TEMPLATE, RULESET_VERSION).unwrap();
    let ids: Vec<String> = ruleset.rules.iter().map(|r| r.id.to_string()).collect();
    assert_eq!(ruleset.rules.len(), 27);
    assert_eq!(ids[0], "0");
    assert_eq!(ids.last().map(String::as_str), Some("14"));
    assert!(ruleset.overrides.is_empty());
  }

  #[test]
  fn bundled_manifest_is_valid() {
    let manifest = compose(None).unwrap().manifest;
    assert_eq!(manifest.ruleset_version, RULESET_VERSION);
    assert_eq!(manifest.rule_count, 27);
    assert!(manifest.rules.iter().all(|r| !r.body_checksum.is_empty()));
    assert!(manifest.overrides.is_empty());
  }

  #[test]
  fn core_only_build_is_verbatim_template() {
    let out = compose(None).unwrap();
    assert_eq!(out.markdown, CORE_TEMPLATE);
  }

  #[test]
  fn compose_selects_fragments() {
    let out = compose(Some("wasm,tauri")).unwrap();
    let ids: Vec<String> = out.ruleset.rules.iter().map(|r| r.id.to_string()).collect();
    assert!(ids.contains(&"WASM-1.1".to_string()));
    assert!(ids.contains(&"TAURI-1.1".to_string()));
    assert!(out.markdown.contains("## WASM-1. WebAssembly Targets"));
    assert!(
      out
        .markdown
        .contains("### TAURI-1.1 Core / Shell Separation")
    );
  }

  #[test]
  fn unknown_template_errors() {
    assert!(compose(Some("nope")).is_err());
    assert!(compose(Some("wasm,nope")).is_err());
  }
}
