//! Filesystem-based resource provider for native platforms.
//!
//! This provider loads resources from the local filesystem with security
//! measures to prevent path traversal attacks.
//!
//! # Security
//!
//! The provider validates that all resolved paths remain within the base path
//! to prevent directory traversal attacks (e.g., `../../../etc/passwd`).

use petty_traits::{ResourceError, ResourceProvider, SharedResourceData};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A resource provider that loads resources from the local filesystem.
///
/// Resources are loaded relative to a base path, which is typically
/// the directory containing the template file.
///
/// # Security
///
/// This provider prevents path traversal attacks by canonicalizing paths
/// and verifying they remain within the base directory. Attempts to access
/// files outside the base path will return a `NotFound` error.
#[derive(Debug)]
pub struct FilesystemResourceProvider {
    base_path: PathBuf,
    /// Canonicalized base path for security checks
    canonical_base: Option<PathBuf>,
}

impl FilesystemResourceProvider {
    /// Creates a new filesystem resource provider with the given base path.
    ///
    /// All resource paths will be resolved relative to this base path.
    /// The base path is canonicalized to enable security checks.
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        let base = base_path.as_ref().to_path_buf();
        // Try to canonicalize for security checks; may fail if path doesn't exist yet
        let canonical = base.canonicalize().ok();
        Self {
            base_path: base,
            canonical_base: canonical,
        }
    }

    /// Returns the base path for this provider.
    pub fn base(&self) -> &Path {
        &self.base_path
    }

    /// Resolves and validates a resource path relative to the base path.
    ///
    /// Returns `None` if the path would escape the base directory (path traversal attack).
    fn resolve_path_safe(&self, path: &str) -> Option<PathBuf> {
        // Reject absolute paths
        if Path::new(path).is_absolute() {
            return None;
        }

        let full_path = self.base_path.join(path);

        // Try to canonicalize and verify it's within base
        if let Ok(canonical) = full_path.canonicalize()
            && let Some(ref base) = self.canonical_base {
                if canonical.starts_with(base) {
                    return Some(canonical);
                }
                // Path escapes base directory - potential attack
                return None;
            }

        // If canonicalization fails (file doesn't exist), do basic path component check
        // This prevents obvious traversal like "../../../etc/passwd"
        for component in Path::new(path).components() {
            if let std::path::Component::ParentDir = component {
                // Contains ".." - reject for safety
                return None;
            }
        }

        Some(full_path)
    }
}

impl ResourceProvider for FilesystemResourceProvider {
    fn load(&self, path: &str) -> Result<SharedResourceData, ResourceError> {
        let full_path = self.resolve_path_safe(path)
            .ok_or_else(|| ResourceError::NotFound(format!(
                "{} (path traversal blocked)", path
            )))?;

        std::fs::read(&full_path)
            .map(Arc::new)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ResourceError::NotFound(path.to_string())
                } else {
                    ResourceError::LoadFailed {
                        path: path.to_string(),
                        message: e.to_string(),
                    }
                }
            })
    }

    fn exists(&self, path: &str) -> bool {
        self.resolve_path_safe(path)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    fn base_path(&self) -> Option<&str> {
        self.base_path.to_str()
    }

    fn name(&self) -> &'static str {
        "FilesystemResourceProvider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_filesystem_provider_load_existing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, b"Hello, World!").unwrap();

        let provider = FilesystemResourceProvider::new(dir.path());
        let data = provider.load("test.txt").unwrap();
        assert_eq!(&*data, b"Hello, World!");
    }

    #[test]
    fn test_filesystem_provider_not_found() {
        let dir = tempdir().unwrap();
        let provider = FilesystemResourceProvider::new(dir.path());

        let result = provider.load("nonexistent.txt");
        assert!(matches!(result, Err(ResourceError::NotFound(_))));
    }

    #[test]
    fn test_filesystem_provider_exists() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("exists.txt");
        fs::write(&file_path, b"").unwrap();

        let provider = FilesystemResourceProvider::new(dir.path());
        assert!(provider.exists("exists.txt"));
        assert!(!provider.exists("not_exists.txt"));
    }

    #[test]
    fn test_filesystem_provider_base_path() {
        let dir = tempdir().unwrap();
        let provider = FilesystemResourceProvider::new(dir.path());

        assert!(provider.base_path().is_some());
    }

    // Security tests for path traversal prevention

    #[test]
    fn test_filesystem_provider_blocks_path_traversal() {
        let dir = tempdir().unwrap();
        let provider = FilesystemResourceProvider::new(dir.path());

        // Attempt path traversal attack
        let result = provider.load("../../../etc/passwd");
        assert!(result.is_err());

        // Should also not exist
        assert!(!provider.exists("../../../etc/passwd"));
    }

    #[test]
    fn test_filesystem_provider_blocks_absolute_paths() {
        let dir = tempdir().unwrap();
        let provider = FilesystemResourceProvider::new(dir.path());

        // Attempt to load absolute path
        let result = provider.load("/etc/passwd");
        assert!(result.is_err());

        assert!(!provider.exists("/etc/passwd"));
    }

    #[test]
    fn test_filesystem_provider_blocks_double_dots() {
        let dir = tempdir().unwrap();
        let provider = FilesystemResourceProvider::new(dir.path());

        // Various forms of path traversal
        assert!(!provider.exists(".."));
        assert!(!provider.exists("foo/../../../bar"));
        assert!(!provider.exists("./../../secret"));
    }

    #[test]
    fn test_filesystem_provider_allows_nested_paths() {
        let dir = tempdir().unwrap();

        // Create nested directory structure
        let nested_dir = dir.path().join("subdir");
        fs::create_dir(&nested_dir).unwrap();
        let file_path = nested_dir.join("nested.txt");
        fs::write(&file_path, b"nested content").unwrap();

        let provider = FilesystemResourceProvider::new(dir.path());

        // Should allow legitimate nested paths
        assert!(provider.exists("subdir/nested.txt"));
        let data = provider.load("subdir/nested.txt").unwrap();
        assert_eq!(&*data, b"nested content");
    }
}
