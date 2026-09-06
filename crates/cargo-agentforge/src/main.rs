//! `cargo agentforge` — install, check, validate, and diff the
//! `AGENTS-RUST.md` constitution for Rust projects.
//!
//! The binary follows the `cargo` subcommand convention: it is invoked as
//! `cargo agentforge <subcommand>`. With no subcommand it defaults to `init`.

use std::path::PathBuf;

use agentforge_builder::{CORE_TEMPLATE, GENERATED_AT, RULESET_VERSION};
use agentforge_core::{Config, CoreError, ExitCode, RealFs, check_status, diff_manifests, install};
use agentforge_domain::{RuleManifest, parse_agents_md, validate_agents_md};

use clap::{Args, Parser, Subcommand};

const AGENTS_FILE: &str = "AGENTS-RUST.md";
const MANIFEST_FILE: &str = ".agentforge.json";

#[derive(Parser)]
#[command(
  name = "cargo-agentforge",
  version,
  about = "Install, check, validate, and diff AGENTS-RUST.md in Rust projects"
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
  Check(OutputArgs),
  /// Print the CLI version. Never touches the network or the filesystem.
  Version,
  /// List the available domain template fragments and their descriptions.
  Templates,
  /// Validate the project's AGENTS-RUST.md and report every issue found.
  Validate(OutputArgs),
  /// Show a rule-level diff between the installed and target rulesets.
  Diff(OutputArgs),
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

/// Shared flags for reporting subcommands.
#[derive(Args, Default)]
struct OutputArgs {
  /// Emit machine-readable JSON on stdout.
  #[arg(long)]
  json: bool,
}

fn main() {
  let cli = Cli::parse();

  let exit = match cli.command.unwrap_or(Command::Init(InitArgs::default())) {
    Command::Init(args) => run_init(&args),
    Command::Check(args) => run_check(&args),
    Command::Version => {
      println!("cargo-agentforge {}", env!("CARGO_PKG_VERSION"));
      ExitCode::Installed
    }
    Command::Templates => run_templates(),
    Command::Validate(args) => run_validate(&args),
    Command::Diff(args) => run_diff(&args),
  };

  std::process::exit(exit.as_i32());
}

fn run_init(args: &InitArgs) -> ExitCode {
  let fragments = match resolve_selection(args.template.as_deref()) {
    Ok(f) => f,
    Err(e) => {
      eprintln!("error: {e}");
      return ExitCode::InputError;
    }
  };
  let output = match compose(&fragments) {
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

fn run_check(args: &OutputArgs) -> ExitCode {
  let manifest = match compose(&[]) {
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
      if args.json {
        print_json(&status);
      } else {
        print_check_status(&status);
      }
      status.exit_code()
    }
    Err(e) => print_error_and_exit(e),
  }
}

fn run_validate(args: &OutputArgs) -> ExitCode {
  let md = match std::fs::read_to_string(AGENTS_FILE) {
    Ok(m) => m,
    Err(_) => {
      eprintln!(
        "✗ {AGENTS_FILE} not found. Run `cargo agentforge init` to install the constitution."
      );
      return ExitCode::NotInstalled;
    }
  };

  let report = validate_agents_md(&md);

  if args.json {
    print_json(&report);
  } else {
    for issue in &report.issues {
      eprintln!(
        "✗ line {}: [{}] {}",
        issue.line,
        issue.kind.kind_label(),
        issue.message
      );
    }
    if report.issue_count == 0 {
      println!(
        "✓ {AGENTS_FILE} is valid: {} rules, no issues.",
        report.rule_count
      );
    } else {
      eprintln!(
        "✗ {AGENTS_FILE} has {} issue(s) across {} rules.",
        report.issue_count, report.rule_count
      );
    }
  }

  if report.issue_count == 0 {
    ExitCode::Installed
  } else {
    ExitCode::InputError
  }
}

fn run_diff(args: &OutputArgs) -> ExitCode {
  let md = match std::fs::read_to_string(AGENTS_FILE) {
    Ok(m) => m,
    Err(_) => {
      eprintln!("✗ {AGENTS_FILE} not found. Run `cargo agentforge init` first.");
      return ExitCode::NotInstalled;
    }
  };

  let installed_rs = match parse_agents_md(&md, RULESET_VERSION) {
    Ok(rs) => rs,
    Err(e) => {
      eprintln!("✗ failed to parse {AGENTS_FILE}: {e}");
      return ExitCode::InputError;
    }
  };

  let fragments = detect_selection(&installed_rs);
  let target = match compose(&fragments) {
    Ok(o) => o,
    Err(e) => {
      eprintln!("error: {e}");
      return ExitCode::InputError;
    }
  };

  let installed_manifest = match RuleManifest::from_rule_set(&installed_rs, GENERATED_AT) {
    Ok(m) => m,
    Err(e) => {
      eprintln!("error: failed to build installed manifest: {e}");
      return ExitCode::InternalError;
    }
  };

  let report = diff_manifests(&installed_manifest, &target.manifest);

  if args.json {
    print_json(&report);
  } else {
    print_diff_report(&report);
  }

  if report.edited + report.added + report.removed == 0 {
    ExitCode::Installed
  } else {
    ExitCode::HasDiff
  }
}

/// Resolve a comma-separated `--template` selection into builder fragments.
fn resolve_selection(selection: Option<&str>) -> Result<Vec<(&'static str, &'static str)>, String> {
  match selection {
    Some(sel) => agentforge_builder::resolve_selection(sel)
      .map_err(|unknown| format!("unknown template: {unknown}")),
    None => Ok(Vec::new()),
  }
}

/// Compose the bundled ruleset with the given fragments. Core-only builds
/// return the verbatim core template; composed builds are re-rendered
/// deterministically.
fn compose(
  fragments: &[(&'static str, &'static str)],
) -> Result<agentforge_builder::BuildOutput, String> {
  agentforge_builder::build(&agentforge_builder::BuildConfig {
    core_template: CORE_TEMPLATE,
    fragments,
    version: RULESET_VERSION,
    generated_at: GENERATED_AT,
  })
  .map_err(|e| format!("failed to compose ruleset: {e}"))
}

/// Detect which shipped templates are present in an installed ruleset by
/// looking for their namespaced rule ids (`WASM-1.1`, `TAURI-1.1`, …).
fn detect_selection(installed: &agentforge_domain::RuleSet) -> Vec<(&'static str, &'static str)> {
  agentforge_builder::TEMPLATES
    .iter()
    .filter(|t| {
      let ns = format!("{}-", t.name.to_ascii_uppercase());
      installed
        .rules
        .iter()
        .any(|r| r.id.as_str().starts_with(&ns))
    })
    .map(|t| (t.name, t.markdown))
    .collect()
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

fn print_json(value: &impl serde::Serialize) {
  match serde_json::to_string_pretty(value) {
    Ok(json) => println!("{json}"),
    Err(e) => eprintln!("error: failed to serialize output: {e}"),
  }
}

fn print_check_status(status: &agentforge_core::CheckStatus) {
  use agentforge_core::CheckStatus;
  match status {
    CheckStatus::UpToDate => {
      println!("✓ {} is up to date.", AGENTS_FILE);
    }
    CheckStatus::Stale { installed, bundled } => {
      eprintln!(
        "✗ {} is stale: installed ruleset {installed}, bundled ruleset {bundled}.",
        AGENTS_FILE
      );
    }
    CheckStatus::NotInstalled => {
      eprintln!(
        "✗ {MANIFEST_FILE} not found. Run `cargo agentforge init` to install the constitution."
      );
    }
  }
}

fn print_diff_report(report: &agentforge_core::DiffReport) {
  use agentforge_core::Change;
  for rule in &report.rules {
    let (mark, verb) = match rule.change {
      Change::Edited => ("~", "edited locally"),
      Change::Added => ("+", "missing from installed"),
      Change::Removed => ("-", "not in target"),
      Change::Unchanged => continue,
    };
    println!(
      "{mark} §{id} {title} ({verb})",
      id = rule.id,
      title = rule.title,
      verb = verb,
      mark = mark
    );
  }
  println!(
    "summary: {} unchanged, {} edited, {} added, {} removed",
    report.unchanged, report.edited, report.added, report.removed
  );
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

trait KindLabel {
  fn kind_label(&self) -> &'static str;
}

impl KindLabel for agentforge_domain::ValidationIssueKind {
  fn kind_label(&self) -> &'static str {
    use agentforge_domain::ValidationIssueKind as K;
    match self {
      K::EmptyRuleSet => "empty-rule-set",
      K::MalformedHeading => "malformed-heading",
      K::DuplicateRuleId => "duplicate-rule-id",
      K::DuplicateSection => "duplicate-section",
      K::DuplicateOverride => "duplicate-override",
      K::MalformedOverride => "malformed-override",
      K::OrphanOverride => "orphan-override",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bundled_template_parses_all_sections() {
    let ruleset = parse_agents_md(CORE_TEMPLATE, RULESET_VERSION).unwrap();
    let ids: Vec<String> = ruleset.rules.iter().map(|r| r.id.to_string()).collect();
    assert_eq!(ruleset.rules.len(), 27);
    assert_eq!(ids[0], "0");
    assert_eq!(ids.last().map(String::as_str), Some("14"));
    assert!(ruleset.overrides.is_empty());
  }

  #[test]
  fn bundled_manifest_is_valid() {
    let manifest = compose(&[]).unwrap().manifest;
    assert_eq!(manifest.ruleset_version, RULESET_VERSION);
    assert_eq!(manifest.rule_count, 27);
    assert!(manifest.rules.iter().all(|r| !r.body_checksum.is_empty()));
    assert!(manifest.overrides.is_empty());
  }

  #[test]
  fn core_only_build_is_verbatim_template() {
    let out = compose(&[]).unwrap();
    assert_eq!(out.markdown, CORE_TEMPLATE);
  }

  #[test]
  fn compose_selects_fragments() {
    let frags = agentforge_builder::resolve_selection("wasm,tauri").unwrap();
    let out = compose(&frags).unwrap();
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
    assert!(resolve_selection(Some("nope")).is_err());
    assert!(resolve_selection(Some("wasm,nope")).is_err());
  }

  #[test]
  fn detect_selection_finds_fragments() {
    let frags = agentforge_builder::resolve_selection("wasm,tauri").unwrap();
    let composed = compose(&frags).unwrap();
    let detected = detect_selection(&composed.ruleset);
    let names: Vec<&str> = detected.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, vec!["wasm", "tauri"]);
  }

  #[test]
  fn detect_selection_core_only_is_empty() {
    let composed = compose(&[]).unwrap();
    assert!(detect_selection(&composed.ruleset).is_empty());
  }

  #[test]
  fn validate_reports_issues_on_core_template() {
    let report = validate_agents_md(CORE_TEMPLATE);
    assert_eq!(report.issue_count, 0);
    assert_eq!(report.rule_count, 27);
  }
}
