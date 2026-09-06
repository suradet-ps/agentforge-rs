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
    std::fs::write(path, contents).map_err(|e| format!("{e}"))
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
