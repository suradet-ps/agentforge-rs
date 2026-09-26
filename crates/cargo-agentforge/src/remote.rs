//! HTTP transport and checksum-source helpers for `update-rules`.
//!
//! TLS validation comes from rustls with Mozilla's roots and is never
//! disabled on this code path. This module stays deliberately thin: every
//! decision that can lose data lives in `agentforge-core`.

use std::time::Duration;

use agentforge_core::{Checksum, Fetcher};
use ureq::Agent;

/// Errors from fetching over the network or locating checksums.
#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
  #[error("invalid ruleset URL: {0}")]
  InvalidUrl(String),

  #[error("checksum entry for {0} is missing from SHA256SUMS.txt")]
  MissingChecksumEntry(String),

  #[error("malformed checksum entry for {0} in SHA256SUMS.txt")]
  MalformedChecksumEntry(String),
}

/// Blocking HTTPS fetcher with a bounded global timeout.
pub struct UreqFetcher {
  agent: Agent,
}

impl UreqFetcher {
  /// Create a fetcher with the default rustls provider and Mozilla roots.
  pub fn new() -> Self {
    let config = Agent::config_builder()
      .timeout_global(Some(Duration::from_secs(60)))
      .build();
    Self {
      agent: config.new_agent(),
    }
  }
}

impl Default for UreqFetcher {
  fn default() -> Self {
    Self::new()
  }
}

impl Fetcher for UreqFetcher {
  fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
      return Err(format!("invalid URL `{url}`: only http(s) is supported"));
    }

    let mut response = self
      .agent
      .get(url)
      .call()
      .map_err(|e| format!("GET {url}: {e}"))?;

    response
      .body_mut()
      .read_to_vec()
      .map_err(|e| format!("GET {url}: {e}"))
  }
}

/// File name of a release asset URL (query and fragment stripped).
///
/// # Errors
///
/// Returns [`RemoteError::InvalidUrl`] when the URL has no final path
/// segment.
pub fn asset_filename(url: &str) -> Result<String, RemoteError> {
  let path = url.split(['?', '#']).next().unwrap_or(url);
  let after_scheme = path
    .strip_prefix("https://")
    .or_else(|| path.strip_prefix("http://"))
    .unwrap_or(path);
  let name = after_scheme
    .split_once('/')
    .map(|(_, rest)| {
      rest
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
    })
    .unwrap_or_default();

  if name.is_empty() {
    Err(RemoteError::InvalidUrl(url.to_string()))
  } else {
    Ok(name.to_string())
  }
}

/// The `SHA256SUMS.txt` that sits next to a release asset.
///
/// # Errors
///
/// Returns [`RemoteError::InvalidUrl`] for URLs without a final path segment.
pub fn sums_url_for(url: &str) -> Result<String, RemoteError> {
  let name = asset_filename(url)?;
  let path = url.split(['?', '#']).next().unwrap_or(url);
  let path = path.trim_end_matches('/');
  let base = &path[..path.len() - name.len()];
  Ok(format!("{base}SHA256SUMS.txt"))
}

/// Find the SHA-256 entry for `filename` in a `SHA256SUMS.txt` payload.
///
/// Understands the standard `<hex>  <name>` and `<hex> *<name>` forms,
/// CRLF line endings, blank lines, and `#` comments.
///
/// # Errors
///
/// Returns [`RemoteError::MissingChecksumEntry`] when the file has no entry
/// for `filename`, and [`RemoteError::MalformedChecksumEntry`] when the
/// entry's digest is not a SHA-256 hex string.
pub fn parse_sha256sums(text: &str, filename: &str) -> Result<Checksum, RemoteError> {
  for line in text.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }

    let mut fields = line.split_whitespace();
    let (Some(hex), Some(name)) = (fields.next(), fields.next()) else {
      continue;
    };
    let name = name.strip_prefix('*').unwrap_or(name);

    if name == filename {
      return Checksum::parse(hex)
        .map_err(|_| RemoteError::MalformedChecksumEntry(filename.to_string()));
    }
  }

  Err(RemoteError::MissingChecksumEntry(filename.to_string()))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn asset_filename_strips_query_and_fragment() {
    assert_eq!(
      asset_filename("https://example.com/releases/agentforge-rules-0.2.0.json").unwrap(),
      "agentforge-rules-0.2.0.json"
    );
    assert_eq!(
      asset_filename("https://example.com/a/b.json?download=1#frag").unwrap(),
      "b.json"
    );
  }

  #[test]
  fn asset_filename_rejects_empty_path() {
    assert!(matches!(
      asset_filename("https://example.com/"),
      Err(RemoteError::InvalidUrl(_))
    ));
  }

  #[test]
  fn sums_url_replaces_the_file_name() {
    assert_eq!(
      sums_url_for("https://example.com/releases/agentforge-rules-0.2.0.json?x=1").unwrap(),
      "https://example.com/releases/SHA256SUMS.txt"
    );
  }

  #[test]
  fn sha256sums_finds_the_entry() {
    let digest = Checksum::of(b"bundle");
    let text = format!(
      "1111111111111111111111111111111111111111111111111111111111111111  other.json\n\
       {}  agentforge-rules-0.2.0.json\n",
      digest.as_str()
    );
    assert_eq!(
      parse_sha256sums(&text, "agentforge-rules-0.2.0.json").unwrap(),
      digest
    );
  }

  #[test]
  fn sha256sums_handles_binary_marker_crlf_and_comments() {
    let digest = Checksum::of(b"bundle");
    let text = format!(
      "# generated by the release workflow\r\n\r\n{} *rules.json\r\n",
      digest.as_str().to_ascii_uppercase()
    );
    assert_eq!(parse_sha256sums(&text, "rules.json").unwrap(), digest);
  }

  #[test]
  fn sha256sums_missing_entry_errors() {
    let digest = Checksum::of(b"bundle");
    let text = format!("{}  other.json\n", digest.as_str());
    assert!(matches!(
      parse_sha256sums(&text, "rules.json"),
      Err(RemoteError::MissingChecksumEntry(_))
    ));
  }

  #[test]
  fn sha256sums_malformed_digest_errors() {
    let text = "not-a-digest  rules.json\n";
    assert!(matches!(
      parse_sha256sums(text, "rules.json"),
      Err(RemoteError::MalformedChecksumEntry(_))
    ));
  }

  #[test]
  fn fetcher_rejects_non_http_urls_without_network() {
    let err = UreqFetcher::new().fetch("ftp://example.com/x").unwrap_err();
    assert!(err.contains("http(s)"), "{err}");
  }
}
