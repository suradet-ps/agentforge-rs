use std::path::Path;

/// A file-system abstraction for install/upgrade operations.
///
/// All I/O goes through this trait so the install flow can be unit-tested
/// against an in-memory mock without touching real files.
///
/// # Contract
///
/// - `read_file` returns `None` if the path does not exist.
/// - `write_file` creates or overwrites the file atomically (from the
///   caller's perspective; the mock does not need true atomics).
/// - `file_exists` is a fast existence check (no content read).
pub trait InstallTarget {
  /// Read the entire contents of a file, or `None` if it does not exist.
  fn read_file(&self, path: &Path) -> Option<String>;

  /// Write `contents` to `path`, creating parent directories if needed.
  fn write_file(&self, path: &Path, contents: &str) -> Result<(), String>;

  /// Check whether a file exists.
  fn file_exists(&self, path: &Path) -> bool;

  /// Return a human-readable label for this target (e.g. the root directory).
  fn label(&self) -> &str;
}

/// Real filesystem implementation backed by `std::fs`.
#[allow(dead_code)]
pub struct RealFs;

impl InstallTarget for RealFs {
  fn read_file(&self, path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
  }

  fn write_file(&self, path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
    }

    // Atomic replace: write a sibling temp file, then rename it over the
    // target. Readers see either the old file or the new one, never a
    // half-written mix, and a failed write leaves the target untouched.
    let file_name = path
      .file_name()
      .ok_or_else(|| format!("invalid path: {}", path.display()))?
      .to_string_lossy();
    let temp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));

    std::fs::write(&temp, contents).map_err(|e| {
      let _ = std::fs::remove_file(&temp);
      e.to_string()
    })?;

    std::fs::rename(&temp, path).map_err(|e| {
      let _ = std::fs::remove_file(&temp);
      e.to_string()
    })
  }

  fn file_exists(&self, path: &Path) -> bool {
    path.exists()
  }

  fn label(&self) -> &str {
    "real filesystem"
  }
}

/// In-memory filesystem for testing.
///
/// Unlike the real filesystem, writes are captured so tests can assert on
/// exactly what the install flow persisted.
#[cfg(test)]
pub struct MockFs {
  files: std::cell::RefCell<std::collections::HashMap<std::path::PathBuf, String>>,
  label: String,
}

#[cfg(test)]
impl MockFs {
  pub fn new() -> Self {
    Self {
      files: std::cell::RefCell::new(std::collections::HashMap::new()),
      label: "mock filesystem".into(),
    }
  }

  pub fn with_file(self, path: impl Into<std::path::PathBuf>, contents: impl Into<String>) -> Self {
    self.files.borrow_mut().insert(path.into(), contents.into());
    self
  }
}

#[cfg(test)]
impl Default for MockFs {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
impl InstallTarget for MockFs {
  fn read_file(&self, path: &Path) -> Option<String> {
    self.files.borrow().get(path).cloned()
  }

  fn write_file(&self, path: &Path, contents: &str) -> Result<(), String> {
    self
      .files
      .borrow_mut()
      .insert(path.to_path_buf(), contents.to_string());
    Ok(())
  }

  fn file_exists(&self, path: &Path) -> bool {
    self.files.borrow().contains_key(path)
  }

  fn label(&self) -> &str {
    &self.label
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn real_fs_write_replaces_atomically() {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let dir = std::env::temp_dir().join(format!("agentforge-fs-{}-{nanos}", std::process::id()));
    let path = dir.join("AGENTS-RUST.md");
    let fs = RealFs;

    fs.write_file(&path, "one").unwrap();
    fs.write_file(&path, "two").unwrap();
    assert_eq!(fs.read_file(&path).as_deref(), Some("two"));

    let leftovers: Vec<String> = std::fs::read_dir(&dir)
      .unwrap()
      .filter_map(|entry| entry.ok())
      .map(|entry| entry.file_name().to_string_lossy().into_owned())
      .filter(|name| name.ends_with(".tmp"))
      .collect();
    assert!(
      leftovers.is_empty(),
      "temp files left behind: {leftovers:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
  }
}
