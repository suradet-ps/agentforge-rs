//! HTTP transport and checksum-source helpers for `update-rules`.
//!
//! TLS validation comes from rustls with Mozilla's roots and is never
//! disabled on this code path. This module stays deliberately thin: every
//! decision that can lose data lives in `agentforge-core`.

use std::time::Duration;

use agentforge_core::{Checksum, Fetcher};
use ureq::Agent;

const FETCH_ATTEMPTS: u32 = 3;
const RETRY_DELAY_STEP: Duration = Duration::from_millis(250);

/// GitHub Releases location of this project's ruleset bundles.
pub const RULESET_RELEASES_BASE: &str =
  "https://github.com/suradet-ps/agentforge-rs/releases/download";

/// Errors from fetching over the network or locating checksums.
#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
  #[error("invalid ruleset URL: {0}")]
  InvalidUrl(String),

  #[error("invalid ruleset version `{0}`: expected a semver-like value such as 0.2.0")]
  InvalidRulesetVersion(String),

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

    fetch_with_retries(
      url,
      FETCH_ATTEMPTS,
      |target| get_once(&self.agent, target),
      |attempt, error| {
        eprintln!("  fetch failed ({error}); retrying ({attempt}/{FETCH_ATTEMPTS})");
        std::thread::sleep(RETRY_DELAY_STEP * attempt);
      },
    )
  }
}

/// One HTTP GET. Returns the body bytes or a human-readable transport error.
fn get_once(agent: &Agent, url: &str) -> Result<Vec<u8>, String> {
  let mut response = agent
    .get(url)
    .call()
    .map_err(|e| format!("GET {url}: {e}"))?;
  response
    .body_mut()
    .read_to_vec()
    .map_err(|e| format!("GET {url}: {e}"))
}

/// Fetch with bounded retries.
///
/// The first attempt uses the canonical URL; retries append a marker so a
/// transient failure that an intermediate cache has pinned to the exact URL
/// (observed with GitHub's release download edge) cannot make the fetch fail
/// permanently. Release assets are immutable, so the marker changes nothing
/// about the content.
///
/// `on_retry` receives the failed attempt number and the error, and is
/// expected to log and/or wait before the next attempt.
fn fetch_with_retries(
  url: &str,
  attempts: u32,
  mut get: impl FnMut(&str) -> Result<Vec<u8>, String>,
  mut on_retry: impl FnMut(u32, &str),
) -> Result<Vec<u8>, String> {
  let mut last_error = String::new();

  for attempt in 0..attempts.max(1) {
    let target = if attempt == 0 {
      url.to_string()
    } else {
      retry_url(url, attempt)
    };

    match get(&target) {
      Ok(bytes) => return Ok(bytes),
      Err(error) => {
        last_error = error;
        if attempt + 1 < attempts.max(1) {
          on_retry(attempt + 1, &last_error);
        }
      }
    }
  }

  if attempts > 1 {
    Err(format!("{last_error} (after {attempts} attempts)"))
  } else {
    Err(last_error)
  }
}

/// The canonical URL for a retry: the same target plus a `url_retry` marker.
fn retry_url(url: &str, attempt: u32) -> String {
  let separator = if url.contains('?') { '&' } else { '?' };
  format!("{url}{separator}url_retry={attempt}")
}

/// Pinned URL of the ruleset bundle published for `version`.
///
/// # Errors
///
/// Returns [`RemoteError::InvalidRulesetVersion`] when the version is empty
/// or contains characters that could escape the release path.
pub fn pinned_ruleset_url(version: &str) -> Result<String, RemoteError> {
  let valid = !version.is_empty()
    && !version.contains("..")
    && version
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
  if !valid {
    return Err(RemoteError::InvalidRulesetVersion(version.to_string()));
  }

  Ok(format!(
    "{RULESET_RELEASES_BASE}/rules-v{version}/agentforge-rules-{version}.json"
  ))
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

  #[test]
  fn pinned_url_targets_the_release_bundle() {
    assert_eq!(
      pinned_ruleset_url("0.2.0").unwrap(),
      "https://github.com/suradet-ps/agentforge-rs/releases/download/rules-v0.2.0/agentforge-rules-0.2.0.json"
    );
  }

  #[test]
  fn pinned_url_rejects_path_escapes() {
    for bad in ["", "..", "0.2.0/../../evil", "0.2.0 evil", "0.2.0?x=y"] {
      assert!(
        matches!(
          pinned_ruleset_url(bad),
          Err(RemoteError::InvalidRulesetVersion(_))
        ),
        "{bad:?} should be rejected"
      );
    }
  }

  #[test]
  fn retry_url_appends_with_question_or_ampersand() {
    assert_eq!(
      retry_url("https://example.com/rules.json", 1),
      "https://example.com/rules.json?url_retry=1"
    );
    assert_eq!(
      retry_url("https://example.com/rules.json?x=1", 2),
      "https://example.com/rules.json?x=1&url_retry=2"
    );
  }

  #[test]
  fn retries_transient_failures_with_a_cache_buster() {
    let mut urls: Vec<String> = Vec::new();
    let mut retries: Vec<(u32, String)> = Vec::new();

    let result = fetch_with_retries(
      "https://example.com/rules.json",
      3,
      |target| {
        urls.push(target.to_string());
        if urls.len() == 1 {
          Err("http status: 500".into())
        } else {
          Ok(b"bundle".to_vec())
        }
      },
      |attempt, error| retries.push((attempt, error.to_string())),
    );

    assert_eq!(result.unwrap(), b"bundle");
    assert_eq!(
      urls,
      vec![
        "https://example.com/rules.json",
        "https://example.com/rules.json?url_retry=1"
      ]
    );
    assert_eq!(retries, vec![(1, "http status: 500".to_string())]);
  }

  #[test]
  fn gives_up_after_the_last_attempt() {
    let mut calls = 0;
    let result = fetch_with_retries(
      "https://example.com/rules.json",
      3,
      |_| {
        calls += 1;
        Err("http status: 500".into())
      },
      |_, _| {},
    );

    let error = result.unwrap_err();
    assert!(error.contains("after 3 attempts"), "{error}");
    assert_eq!(calls, 3);
  }

  #[test]
  #[ignore = "live network: requires the published rules-v0.1.0 ruleset release"]
  fn live_fetch_of_the_published_bundle() {
    let url = pinned_ruleset_url("0.1.0").unwrap();
    let bytes = UreqFetcher::new().fetch(&url).unwrap();
    let bundle = crate::bundle::decode_bundle(&bytes).unwrap();
    crate::bundle::verify_bundle(&bundle).unwrap();
    assert_eq!(bundle.ruleset_version, "0.1.0");
  }
}
