//! `cargo agentforge`: install, check, validate, diff, and update the
//! `AGENTS-RUST.md` constitution for Rust projects.
//!
//! The binary follows the `cargo` subcommand convention: it is invoked as
//! `cargo agentforge <subcommand>`. With no subcommand it defaults to `init`.

mod bundle;
mod remote;

use std::io::{IsTerminal, Write as _};
use std::path::PathBuf;

use agentforge_builder::{CORE_TEMPLATE, GENERATED_AT, RULESET_VERSION};
use agentforge_core::{
  Checksum, Config, CoreError, ExitCode, Fetcher, Outcome, RealFs, check_status, diff_manifests,
  install, update,
};
use agentforge_domain::{RuleManifest, parse_agents_md, validate_agents_md};

use clap::{Args, Parser, Subcommand};
use remote::UreqFetcher;

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
  /// Run the validation pipeline on the bundled ruleset (the release gate).
  Verify(VerifyArgs),
  /// Download, verify, and apply a newer ruleset bundle over the installed one.
  UpdateRules(UpdateRulesArgs),
  /// Write the embedded ruleset as a release bundle (used by the release workflow).
  Bundle(BundleArgs),
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

/// Flags for `verify` (the build validation pipeline).
#[derive(Args, Default)]
struct VerifyArgs {
  /// Comma-separated domain templates to verify, e.g. `--template wasm,tauri`.
  #[arg(long, value_name = "TEMPLATES")]
  template: Option<String>,
  /// Emit machine-readable JSON on stdout.
  #[arg(long)]
  json: bool,
}

/// Flags for `update-rules`.
#[derive(Args)]
struct UpdateRulesArgs {
  /// URL of the ruleset bundle to download.
  #[arg(long, value_name = "URL")]
  url: String,
  /// Expected SHA-256 of the bundle; defaults to SHA256SUMS.txt next to it.
  #[arg(long, value_name = "SHA256")]
  sha256: Option<String>,
  /// Skip the interactive confirmation prompt (required when not a terminal).
  #[arg(long)]
  yes: bool,
  /// Overwrite locally-edited rules instead of reporting a conflict.
  #[arg(long)]
  force: bool,
  /// Report what would change without fetching or writing anything.
  #[arg(long)]
  dry_run: bool,
  /// Emit machine-readable JSON on stdout.
  #[arg(long)]
  json: bool,
}

/// Flags for `bundle`.
#[derive(Args)]
struct BundleArgs {
  /// File to write the bundle to.
  #[arg(long, value_name = "PATH")]
  output: PathBuf,
}

fn main() {
  // Cargo invokes external subcommands as `cargo agentforge <args>`, passing
  // the subcommand name itself as the first argument. Strip it so the same
  // CLI parses both through cargo and when invoked directly.
  let cli = parse_cli(std::env::args_os().collect());

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
    Command::Verify(args) => run_verify(&args),
    Command::UpdateRules(args) => run_update_rules(&args),
    Command::Bundle(args) => run_bundle(&args),
  };

  std::process::exit(exit.as_i32());
}

/// Parse the CLI, tolerating the `agentforge` argument that cargo prepends
/// when it runs an external subcommand (`cargo agentforge init` ->
/// `cargo-agentforge agentforge init`).
fn parse_cli(args: Vec<std::ffi::OsString>) -> Cli {
  Cli::parse_from(strip_cargo_subcommand_name(args))
}

/// Remove the leading `agentforge` argument when present. Direct
/// invocations (`cargo-agentforge init`) are left untouched.
fn strip_cargo_subcommand_name(args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
  if args.get(1).is_some_and(|a| a == "agentforge") {
    let mut args = args;
    args.remove(1);
    args
  } else {
    args
  }
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

/// Run the validation pipeline on the bundled ruleset. Non-zero exit when
/// the build reports errors — the release gate.
fn run_verify(args: &VerifyArgs) -> ExitCode {
  let fragments = match resolve_selection(args.template.as_deref()) {
    Ok(f) => f,
    Err(e) => {
      eprintln!("error: {e}");
      return ExitCode::InputError;
    }
  };

  let report = agentforge_builder::validation_report(&agentforge_builder::BuildConfig {
    core_template: CORE_TEMPLATE,
    fragments: &fragments,
    version: RULESET_VERSION,
    generated_at: &resolve_generated_at(),
  });

  if args.json {
    print_json(&report);
  } else {
    println!(
      "rules: {}, fragments: {}",
      report.rule_count, report.fragment_count
    );
    for warning in &report.warnings {
      eprintln!("⚠ {warning}");
    }
    for error in &report.errors {
      eprintln!("✗ {error}");
    }
    if report.errors.is_empty() {
      println!("✓ ruleset is valid and shippable.");
    } else {
      eprintln!(
        "✗ ruleset has {} error(s) and must not be shipped.",
        report.errors.len()
      );
    }
  }

  if report.errors.is_empty() {
    ExitCode::Installed
  } else {
    ExitCode::InputError
  }
}

/// Download, verify, and apply a ruleset bundle. The network is touched only
/// after explicit confirmation (or an explicit `--yes`); `--dry-run` still
/// fetches and verifies but never writes.
fn run_update_rules(args: &UpdateRulesArgs) -> ExitCode {
  let Some(installed_md) = std::fs::read_to_string(AGENTS_FILE).ok() else {
    eprintln!("✗ {AGENTS_FILE} not found. Run `cargo agentforge init` first.");
    return ExitCode::NotInstalled;
  };
  let Some(installed_json) = std::fs::read_to_string(MANIFEST_FILE).ok() else {
    eprintln!("✗ {MANIFEST_FILE} not found. Run `cargo agentforge init` first.");
    return ExitCode::NotInstalled;
  };
  let installed: RuleManifest = match serde_json::from_str(&installed_json) {
    Ok(manifest) => manifest,
    Err(e) => {
      eprintln!("✗ failed to parse {MANIFEST_FILE}: {e}");
      return ExitCode::InputError;
    }
  };
  let installed_rs = match parse_agents_md(&installed_md, &installed.ruleset_version) {
    Ok(ruleset) => ruleset,
    Err(e) => {
      eprintln!("✗ failed to parse {AGENTS_FILE}: {e}");
      return ExitCode::InputError;
    }
  };
  let selection = detect_template_names(&installed_rs);

  if !args.yes && !args.dry_run {
    if !std::io::stdin().is_terminal() {
      eprintln!("✗ refusing to prompt in a non-interactive session; pass --yes");
      return ExitCode::InputError;
    }
    if !prompt_update(&args.url, args.sha256.as_deref()) {
      println!("aborted; nothing written.");
      return ExitCode::Skipped;
    }
  }

  let fetcher = UreqFetcher::new();
  let filename = match remote::asset_filename(&args.url) {
    Ok(name) => name,
    Err(e) => {
      eprintln!("✗ {e}");
      return ExitCode::InputError;
    }
  };

  let bundle_bytes = match fetcher.fetch(&args.url) {
    Ok(bytes) => bytes,
    Err(e) => {
      eprintln!("✗ {e}");
      return ExitCode::InputError;
    }
  };

  let expected =
    match resolve_expected_checksum(&fetcher, args.sha256.as_deref(), &args.url, &filename) {
      Ok(checksum) => checksum,
      Err(e) => {
        eprintln!("✗ {e}");
        return ExitCode::InputError;
      }
    };
  if let Err(e) = expected.verify(&filename, &bundle_bytes) {
    eprintln!("✗ {e}");
    return ExitCode::InputError;
  }

  let bundle = match bundle::decode_bundle(&bundle_bytes) {
    Ok(bundle) => bundle,
    Err(e) => {
      eprintln!("✗ {e}");
      return ExitCode::InputError;
    }
  };
  if let Err(e) = bundle::verify_bundle(&bundle) {
    eprintln!("✗ {e}");
    return ExitCode::InputError;
  }
  let target = match bundle::compose_target(&bundle, &selection) {
    Ok(target) => target,
    Err(e) => {
      eprintln!("✗ {e}");
      return ExitCode::InputError;
    }
  };

  if !args.json {
    println!("✓ {filename} sha256 verified");
    if selection.is_empty() {
      println!("  selection: core only");
    } else {
      println!("  selection: {}", selection.join(", "));
    }
    println!(
      "  ruleset: {} -> {}",
      installed.ruleset_version, bundle.ruleset_version
    );
  }

  let config = Config {
    manifest: target.manifest,
    agents_md: target.markdown,
    agents_md_path: PathBuf::from(AGENTS_FILE),
    manifest_path: PathBuf::from(MANIFEST_FILE),
    force: args.force,
    dry_run: args.dry_run,
  };

  let outcome = match update(&RealFs, &config) {
    Ok(outcome) => outcome,
    Err(e) => return print_error_and_exit(e),
  };

  if args.json {
    let edited = match &outcome {
      Outcome::Conflict { edited_rules } => edited_rules.as_slice(),
      _ => &[],
    };
    print_json(&UpdateReport {
      from: &installed.ruleset_version,
      to: &config.manifest.ruleset_version,
      templates: &selection,
      url: &args.url,
      status: outcome_status(&outcome),
      edited_rules: edited,
    });
  } else {
    print_outcome(&outcome, &config);
  }

  outcome.exit_code()
}

/// Ask before the first network request. The URL and expected checksum are
/// printed so the user confirms exactly what will be downloaded.
fn prompt_update(url: &str, sha256: Option<&str>) -> bool {
  println!("About to download the ruleset bundle:");
  println!("  url: {url}");
  match sha256 {
    Some(hex) => println!("  expected sha256: {hex}"),
    None => println!("  expected sha256: from SHA256SUMS.txt next to the bundle"),
  }
  print!("Proceed? [y/N] ");
  let _ = std::io::stdout().flush();

  let mut answer = String::new();
  if std::io::stdin().read_line(&mut answer).is_err() {
    return false;
  }
  matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Resolve the expected digest of the bundle: an explicit `--sha256`, or the
/// entry for the asset in the neighboring `SHA256SUMS.txt`.
fn resolve_expected_checksum(
  fetcher: &impl Fetcher,
  sha256: Option<&str>,
  url: &str,
  filename: &str,
) -> Result<Checksum, String> {
  if let Some(hex) = sha256 {
    return Checksum::parse(hex).map_err(|e| e.to_string());
  }

  let sums_url = remote::sums_url_for(url).map_err(|e| e.to_string())?;
  let bytes = fetcher.fetch(&sums_url)?;
  let text =
    String::from_utf8(bytes).map_err(|_| format!("{sums_url}: response is not valid UTF-8"))?;
  remote::parse_sha256sums(&text, filename).map_err(|e| e.to_string())
}

/// Write the embedded ruleset as a release bundle for the distribution.
fn run_bundle(args: &BundleArgs) -> ExitCode {
  let bundle = match bundle::shipped_bundle() {
    Ok(bundle) => bundle,
    Err(e) => {
      eprintln!("error: {e}");
      return ExitCode::InternalError;
    }
  };

  let mut json = match serde_json::to_vec_pretty(&bundle) {
    Ok(json) => json,
    Err(e) => {
      eprintln!("error: failed to serialize bundle: {e}");
      return ExitCode::InternalError;
    }
  };
  json.push(b'\n');

  if let Err(e) = std::fs::write(&args.output, json) {
    eprintln!("error: failed to write {}: {e}", args.output.display());
    return ExitCode::InputError;
  }

  println!(
    "✅ Wrote {} (ruleset {})",
    args.output.display(),
    bundle.ruleset_version
  );
  ExitCode::Installed
}

fn outcome_status(outcome: &Outcome) -> &'static str {
  match outcome {
    Outcome::Installed => "installed",
    Outcome::Upgraded => "updated",
    Outcome::Skipped => "up-to-date",
    Outcome::Conflict { .. } => "conflict",
    Outcome::DryRun { .. } => "dry-run",
  }
}

/// Machine-readable `update-rules` report.
#[derive(serde::Serialize)]
struct UpdateReport<'a> {
  from: &'a str,
  to: &'a str,
  templates: &'a [&'a str],
  url: &'a str,
  status: &'a str,
  edited_rules: &'a [String],
}

/// Resolve the manifest `generated_at` timestamp: use `SOURCE_DATE_EPOCH`
/// when the build system pins one (reproducible builds), otherwise the
/// fixed default constant.
fn resolve_generated_at() -> String {
  generated_at_from_env(std::env::var("SOURCE_DATE_EPOCH").ok().as_deref())
}

/// Pure helper so the timestamp logic is testable without touching the
/// process environment.
fn generated_at_from_env(raw: Option<&str>) -> String {
  match raw {
    Some(v) => v
      .trim()
      .parse::<i64>()
      .ok()
      .map(agentforge_builder::generated_at_from_epoch)
      .unwrap_or_else(|| GENERATED_AT.to_string()),
    None => GENERATED_AT.to_string(),
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
    generated_at: &resolve_generated_at(),
  })
  .map_err(|e| format!("failed to compose ruleset: {e}"))
}

/// Detect which shipped templates are present in an installed ruleset by
/// looking for their namespaced rule ids (`WASM-1.1`, `TAURI-1.1`, …).
fn detect_template_names(installed: &agentforge_domain::RuleSet) -> Vec<&'static str> {
  agentforge_builder::TEMPLATES
    .iter()
    .filter(|t| {
      let ns = format!("{}-", t.name.to_ascii_uppercase());
      installed
        .rules
        .iter()
        .any(|r| r.id.as_str().starts_with(&ns))
    })
    .map(|t| t.name)
    .collect()
}

fn detect_selection(installed: &agentforge_domain::RuleSet) -> Vec<(&'static str, &'static str)> {
  detect_template_names(installed)
    .into_iter()
    .filter_map(|name| {
      agentforge_builder::TEMPLATES
        .iter()
        .find(|t| t.name == name)
        .map(|t| (t.name, t.markdown))
    })
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
    E::VersionMismatch { .. } | E::VersionDowngrade { .. } => ExitCode::Conflict,
    E::NotInstalled { .. } => ExitCode::NotInstalled,
    E::ManifestRead { .. }
    | E::ManifestDeserialize(_)
    | E::WriteFailed { .. }
    | E::MarkdownParse { .. }
    | E::ManifestBuild { .. }
    | E::MalformedChecksum { .. }
    | E::ChecksumMismatch { .. } => ExitCode::InputError,
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

  #[test]
  fn bundled_ruleset_passes_validation_pipeline() {
    let report = agentforge_builder::validation_report(&agentforge_builder::BuildConfig {
      core_template: CORE_TEMPLATE,
      fragments: &[],
      version: RULESET_VERSION,
      generated_at: GENERATED_AT,
    });
    assert!(report.errors.is_empty());
    assert_eq!(report.rule_count, 27);
    assert_eq!(report.fragment_count, 0);
  }

  #[test]
  fn source_date_epoch_overrides_timestamp() {
    assert_eq!(
      generated_at_from_env(Some("1700000000")),
      "2023-11-14T22:13:20Z"
    );
    assert_eq!(generated_at_from_env(None), GENERATED_AT);
  }

  #[test]
  fn invalid_source_date_epoch_falls_back() {
    assert_eq!(generated_at_from_env(Some("not-a-number")), GENERATED_AT);
    assert_eq!(
      generated_at_from_env(Some("  1700000000  ")),
      "2023-11-14T22:13:20Z"
    );
  }

  #[test]
  fn strip_cargo_subcommand_name_removes_cargo_prefix() {
    use std::ffi::OsString;
    let via_cargo = strip_cargo_subcommand_name(
      ["cargo-agentforge", "agentforge", "init"]
        .into_iter()
        .map(OsString::from)
        .collect(),
    );
    assert_eq!(via_cargo, ["cargo-agentforge", "init"].map(OsString::from));
  }

  #[test]
  fn strip_cargo_subcommand_name_keeps_direct_invocation() {
    use std::ffi::OsString;
    let direct = strip_cargo_subcommand_name(
      ["cargo-agentforge", "init"]
        .into_iter()
        .map(OsString::from)
        .collect(),
    );
    assert_eq!(direct, ["cargo-agentforge", "init"].map(OsString::from));
  }

  #[test]
  fn parses_cargo_invocation_with_subcommand_name() {
    let cli = parse_cli(
      ["cargo-agentforge", "agentforge", "version"]
        .map(std::ffi::OsString::from)
        .to_vec(),
    );
    assert!(matches!(cli.command, Some(Command::Version)));
  }

  #[test]
  fn parses_direct_invocation_without_subcommand_name() {
    let cli = parse_cli(
      ["cargo-agentforge", "version"]
        .map(std::ffi::OsString::from)
        .to_vec(),
    );
    assert!(matches!(cli.command, Some(Command::Version)));
  }

  #[test]
  fn parses_no_arguments_as_default_init() {
    let cli = parse_cli(["cargo-agentforge"].map(std::ffi::OsString::from).to_vec());
    assert!(cli.command.is_none());
  }

  #[test]
  fn parses_update_rules_flags() {
    let cli = parse_cli(
      [
        "cargo-agentforge",
        "agentforge",
        "update-rules",
        "--url",
        "https://example.com/agentforge-rules-0.2.0.json",
        "--yes",
        "--dry-run",
      ]
      .map(std::ffi::OsString::from)
      .to_vec(),
    );
    match cli.command {
      Some(Command::UpdateRules(args)) => {
        assert_eq!(args.url, "https://example.com/agentforge-rules-0.2.0.json");
        assert!(args.yes);
        assert!(args.dry_run);
        assert!(!args.force);
        assert!(args.sha256.is_none());
      }
      _ => panic!("expected update-rules"),
    }
  }

  #[test]
  fn update_rules_requires_a_url() {
    let parsed = Cli::try_parse_from(
      ["cargo-agentforge", "agentforge", "update-rules"]
        .map(std::ffi::OsString::from)
        .to_vec(),
    );
    assert!(parsed.is_err());
  }
}
