//! The bundled ruleset: the core constitution plus every shipped domain
//! template fragment, all embedded at compile time so the install path
//! never touches the network.

/// The verbatim core constitution.
pub const CORE_TEMPLATE: &str = include_str!("../templates/core.md");

/// Semver of the bundled ruleset, independent of the CLI version.
pub const RULESET_VERSION: &str = "0.1.0";

/// Fixed generation timestamp so repeated builds produce byte-identical
/// manifests (reproducibility).
pub const GENERATED_AT: &str = "2026-01-01T00:00:00Z";

/// A selectable domain template fragment.
pub struct Template {
  /// Stable name used with `cargo agentforge init --template <name>`.
  pub name: &'static str,
  /// One-line description shown by `cargo agentforge templates`.
  pub description: &'static str,
  /// The embedded fragment markdown.
  pub markdown: &'static str,
}

/// Every domain template shipped in-repo.
pub const TEMPLATES: &[Template] = &[
  Template {
    name: "wasm",
    description: "WebAssembly targets: linear memory, wasm-bindgen interop, size budgets",
    markdown: include_str!("../templates/fragments/wasm.md"),
  },
  Template {
    name: "tauri",
    description: "Tauri desktop shells: core/shell split, capabilities, signed updates",
    markdown: include_str!("../templates/fragments/tauri.md"),
  },
  Template {
    name: "bevy",
    description: "Bevy ECS games: systems, components, assets, state",
    markdown: include_str!("../templates/fragments/bevy.md"),
  },
  Template {
    name: "embedded",
    description: "no_std firmware: peripherals, interrupt safety, determinism",
    markdown: include_str!("../templates/fragments/embedded.md"),
  },
  Template {
    name: "axum",
    description: "axum web services: handler shape, error middleware, state",
    markdown: include_str!("../templates/fragments/axum.md"),
  },
  Template {
    name: "cli",
    description: "Command-line apps: typed args, exit codes, config conventions",
    markdown: include_str!("../templates/fragments/cli.md"),
  },
  Template {
    name: "library",
    description: "Library crates: API surface, SemVer, docs, dependency hygiene",
    markdown: include_str!("../templates/fragments/library.md"),
  },
];

/// Look up a template by its stable name.
pub fn get_template(name: &str) -> Option<&'static Template> {
  TEMPLATES.iter().find(|t| t.name == name)
}

/// Resolve a comma-separated selection into fragment tuples for the builder,
/// preserving order and rejecting unknown names.
///
/// # Errors
///
/// Returns `Err(unknown)` with the first unrecognized name.
pub fn resolve_selection(selection: &str) -> Result<Vec<(&'static str, &'static str)>, String> {
  let mut resolved = Vec::new();
  for name in selection
    .split(',')
    .map(str::trim)
    .filter(|s| !s.is_empty())
  {
    match get_template(name) {
      Some(t) => resolved.push((t.name, t.markdown)),
      None => return Err(name.to_string()),
    }
  }
  Ok(resolved)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_template_is_known_by_name() {
    for t in TEMPLATES {
      assert_eq!(get_template(t.name).map(|x| x.name), Some(t.name));
    }
  }

  #[test]
  fn resolve_selection_orders_and_errors() {
    let sel = resolve_selection("wasm,tauri").unwrap();
    assert_eq!(sel.len(), 2);
    assert_eq!(sel[0].0, "wasm");
    assert_eq!(sel[1].0, "tauri");
    assert_eq!(resolve_selection("wasm,nope"), Err("nope".to_string()));
  }

  #[test]
  fn core_template_parses() {
    let rs = agentforge_domain::parse_agents_md(CORE_TEMPLATE, RULESET_VERSION).unwrap();
    assert_eq!(rs.rules.len(), 27);
  }
}
