//! Composes the core constitution with domain template fragments into a
//! validated, deterministic `AGENTS-RUST.md` and its manifest.

mod render;

use agentforge_domain::error::DomainError;
use agentforge_domain::manifest::RuleManifest;
use agentforge_domain::rule::RuleSet;

pub use render::render_agents_md;

/// Errors surfaced while building a composed ruleset.
#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
  #[error("failed to parse {what}: {source}")]
  Parse {
    what: String,
    #[source]
    source: DomainError,
  },

  #[error("fragment `{fragment}` conflicts with the composed ruleset: {source}")]
  FragmentConflict {
    fragment: String,
    #[source]
    source: DomainError,
  },

  #[error("failed to build manifest: {0}")]
  Manifest(#[source] DomainError),
}

/// Everything the builder needs to produce a composed ruleset.
#[derive(Debug)]
pub struct BuildConfig<'a> {
  /// The core constitution markdown.
  pub core_template: &'a str,
  /// Selected domain fragments as `(name, markdown)`, in selection order.
  pub fragments: &'a [(&'a str, &'a str)],
  /// Semver for the composed ruleset.
  pub version: &'a str,
  /// Fixed generation timestamp (reproducibility).
  pub generated_at: &'a str,
}

/// Result of a successful build.
#[derive(Debug)]
pub struct BuildOutput {
  /// The composed, validated rule set (core + fragments).
  pub ruleset: RuleSet,
  /// Manifest derived from the composed rule set.
  pub manifest: RuleManifest,
  /// The final `AGENTS-RUST.md` content. For a core-only build this is the
  /// verbatim core template; with fragments it is re-rendered deterministically.
  pub markdown: String,
}

/// Compose the core constitution with zero or more domain fragments.
///
/// Merge invariants:
/// - no rule-id or section-id collision across fragments or with the core;
/// - every fragment override targets an existing rule in the composed set
///   (fragments may target core rules);
/// - section ordering is deterministic (core first, then fragments in
///   selection order).
///
/// # Errors
///
/// Returns [`BuilderError::Parse`] when any input fails to parse,
/// [`BuilderError::FragmentConflict`] on a merge violation, and
/// [`BuilderError::Manifest`] when manifest generation fails.
pub fn build(cfg: &BuildConfig) -> Result<BuildOutput, BuilderError> {
  let mut rs =
    parse_agents_md(cfg.core_template, cfg.version).map_err(|source| BuilderError::Parse {
      what: "core template".into(),
      source,
    })?;

  for (name, fragment_md) in cfg.fragments {
    let fragment =
      agentforge_domain::parse_agents_md_fragment(fragment_md, cfg.version).map_err(|source| {
        BuilderError::Parse {
          what: format!("fragment `{name}`"),
          source,
        }
      })?;

    for rule in fragment.rules {
      rs.add_rule(rule)
        .map_err(|source| BuilderError::FragmentConflict {
          fragment: (*name).to_string(),
          source,
        })?;
    }
    for section in fragment.sections {
      rs.add_section(section)
        .map_err(|source| BuilderError::FragmentConflict {
          fragment: (*name).to_string(),
          source,
        })?;
    }
    for ovr in fragment.overrides {
      rs.add_override(ovr)
        .map_err(|source| BuilderError::FragmentConflict {
          fragment: (*name).to_string(),
          source,
        })?;
    }
  }

  let manifest =
    RuleManifest::from_rule_set(&rs, cfg.generated_at).map_err(BuilderError::Manifest)?;

  let markdown = if cfg.fragments.is_empty() {
    cfg.core_template.to_string()
  } else {
    render_agents_md(&rs)
  };

  Ok(BuildOutput {
    ruleset: rs,
    manifest,
    markdown,
  })
}

fn parse_agents_md(source: &str, version: &str) -> Result<RuleSet, DomainError> {
  agentforge_domain::parse_agents_md(source, version)
}

#[cfg(test)]
mod tests {
  use super::*;

  const CORE: &str = r#"# AGENTS-RUST.md

## 0. Golden Rules

- Always apply.

## 1. Rust Idioms

### 1.1 Errors

- Use thiserror.

[OVERRIDE §1.1] No — use anyhow.
"#;

  const WASM: &str = r#"## WASM-1. WebAssembly

### WASM-1.1 Memory

- Keep linear memory.

### WASM-1.2 Async

- Use wasm-bindgen.
"#;

  const TAURI: &str = r#"## TAURI-1. Desktop Shell

### TAURI-1.1 Frontend

- Bundle assets.
"#;

  const COLLIDES: &str = r#"## WASM-1. Collision

### WASM-1.1 Memory

- Duplicate id.
"#;

  const ORPHAN_OVERRIDE: &str = r#"## WASM-1. WebAssembly

### WASM-1.1 Memory

- Body.

[OVERRIDE §999.1] No such rule anywhere.
"#;

  const OVERRIDES_CORE: &str = r#"## WASM-1. WebAssembly

### WASM-1.1 Memory

- Keep linear memory.

[OVERRIDE §0] Golden rules still hold but interpreted for wasm.
"#;

  fn config<'a>(fragments: &'a [(&'a str, &'a str)]) -> BuildConfig<'a> {
    BuildConfig {
      core_template: CORE,
      fragments,
      version: "1.0.0",
      generated_at: "2026-01-01T00:00:00Z",
    }
  }

  #[test]
  fn core_only_returns_verbatim_template() {
    let out = build(&config(&[])).unwrap();
    assert_eq!(out.markdown, CORE);
    assert_eq!(out.ruleset.version, "1.0.0");
  }

  #[test]
  fn merges_fragments_in_order() {
    let frags = [("wasm", WASM), ("tauri", TAURI)];
    let out = build(&config(&frags)).unwrap();
    let ids: Vec<String> = out.ruleset.rules.iter().map(|r| r.id.to_string()).collect();
    assert_eq!(ids, vec!["0", "1.1", "WASM-1.1", "WASM-1.2", "TAURI-1.1"]);
    assert_eq!(out.ruleset.overrides.len(), 1);
    assert!(out.markdown.contains("## WASM-1. WebAssembly"));
    assert!(out.markdown.contains("### WASM-1.1 Memory"));
    assert!(out.markdown.contains("### TAURI-1.1 Frontend"));
  }

  #[test]
  fn core_override_survives_composition() {
    let frags = [("wasm", WASM)];
    let out = build(&config(&frags)).unwrap();
    assert_eq!(out.ruleset.overrides[0].target_rule_id.as_str(), "1.1");
  }

  #[test]
  fn rejects_rule_id_collision() {
    let frags = [("wasm", WASM), ("collides", COLLIDES)];
    let err = build(&config(&frags)).unwrap_err();
    assert!(matches!(err, BuilderError::FragmentConflict { .. }));
  }

  #[test]
  fn rejects_orphan_override() {
    let frags = [("wasm", ORPHAN_OVERRIDE)];
    let err = build(&config(&frags)).unwrap_err();
    assert!(matches!(err, BuilderError::FragmentConflict { .. }));
  }

  #[test]
  fn fragment_can_override_core_rule() {
    let frags = [("wasm", OVERRIDES_CORE)];
    let out = build(&config(&frags)).unwrap();
    assert_eq!(out.ruleset.overrides.len(), 2);
    assert!(
      out
        .ruleset
        .overrides
        .iter()
        .any(|o| o.target_rule_id.as_str() == "0")
    );
  }

  #[test]
  fn rejects_malformed_fragment() {
    let frags = [("broken", "## 1\n\nno title body")];
    assert!(matches!(
      build(&config(&frags)),
      Err(BuilderError::Parse { .. })
    ));
  }

  #[test]
  fn deterministic_composed_markdown() {
    let frags = [("wasm", WASM), ("tauri", TAURI)];
    let a = build(&config(&frags)).unwrap().markdown;
    let b = build(&config(&frags)).unwrap().markdown;
    assert_eq!(a, b);
  }

  #[test]
  fn composed_markdown_round_trips() {
    let frags = [("wasm", WASM)];
    let out = build(&config(&frags)).unwrap();
    let reparsed = parse_agents_md(&out.markdown, "1.0.0").unwrap();
    assert_eq!(reparsed.rules.len(), out.ruleset.rules.len());
    assert_eq!(reparsed.overrides.len(), out.ruleset.overrides.len());
    for (a, b) in out.ruleset.rules.iter().zip(reparsed.rules.iter()) {
      assert_eq!(a.id, b.id);
      assert_eq!(a.body, b.body);
    }
  }
}
