//! ResourceProvider trait for abstracting resource loading.
//!
//! This trait allows the engine to load resources (images, templates, etc.)
//! without being tied to filesystem access.

use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;

/// Error type for resource loading operations.
#[derive(Error, Debug, Clone)]
pub enum ResourceError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Failed to load resource '{path}': {message}")]
    LoadFailed { path: String, message: String },

    #[error("Invalid resource format: {0}")]
    InvalidFormat(String),

    #[error("I/O error: {0}")]
    Io(String),
}

impl From<std::io::Error> for ResourceError {
    fn from(err: std::io::Error) -> Self {
        ResourceError::Io(err.to_string())
    }
}

/// Shared resource data type (reference-counted bytes).
pub type SharedResourceData = Arc<Vec<u8>>;

/// A trait for loading resources from various sources.
///
/// This abstraction allows the engine to work with resources from:
/// - Local filesystem
/// - In-memory storage
/// - Remote URLs
/// - Embedded resources
///
/// # Implementations
///
/// - `FilesystemResourceProvider`: Loads from local filesystem (feature-gated)
/// - `InMemoryResourceProvider`: Loads from pre-populated memory (always available)
///
/// # Example
///
/// ```ignore
/// let provider: Box<dyn ResourceProvider> = Box::new(InMemoryResourceProvider::new());
/// provider.add("logo.png", logo_bytes);
/// let data = provider.load("logo.png")?;
/// ```
pub trait ResourceProvider: Send + Sync + Debug {
    /// Load a resource by its path/URI.
    ///
    /// # Arguments
    ///
    /// * `path` - The path or URI of the resource to load
    ///
    /// # Returns
    ///
    /// The resource data as a shared byte vector, or an error if not found.
    fn load(&self, path: &str) -> Result<SharedResourceData, ResourceError>;

    /// Check if a resource exists.
    ///
    /// # Arguments
    ///
    /// * `path` - The path or URI to check
    ///
    /// # Returns
    ///
    /// `true` if the resource exists and can be loaded.
    fn exists(&self, path: &str) -> bool;

    /// Get the base path for resolving relative resources.
    ///
    /// Returns `None` if the provider doesn't use path-based resolution.
    fn base_path(&self) -> Option<&str> {
        None
    }

    /// Returns a human-readable name for this provider (for logging/debugging).
    fn name(&self) -> &'static str;
}

/// An in-memory resource provider.
///
/// Resources are stored in memory and must be pre-populated before use.
/// This is the simplest provider and works in any environment including WASM.
#[derive(Debug, Default)]
pub struct InMemoryResourceProvider {
    resources: std::sync::RwLock<std::collections::HashMap<String, SharedResourceData>>,
}

impl InMemoryResourceProvider {
    pub fn new() -> Self {
        Self {
            resources: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Add a resource to the in-memory store.
    ///
    /// # Arguments
    ///
    /// * `path` - The path/key to store the resource under
    /// * `data` - The resource data
    ///
    /// # Errors
    ///
    /// Returns `ResourceError::LoadFailed` if the internal lock is poisoned.
    pub fn add(&self, path: impl Into<String>, data: Vec<u8>) -> Result<(), ResourceError> {
        let path_string = path.into();
        let mut resources = self
            .resources
            .write()
            .map_err(|_| ResourceError::LoadFailed {
                path: path_string.clone(),
                message: "resource store lock poisoned".to_string(),
            })?;
        resources.insert(path_string, Arc::new(data));
        Ok(())
    }

    /// Add a resource from shared data.
    ///
    /// # Errors
    ///
    /// Returns `ResourceError::LoadFailed` if the internal lock is poisoned.
    pub fn add_shared(
        &self,
        path: impl Into<String>,
        data: SharedResourceData,
    ) -> Result<(), ResourceError> {
        let path_string = path.into();
        let mut resources = self
            .resources
            .write()
            .map_err(|_| ResourceError::LoadFailed {
                path: path_string.clone(),
                message: "resource store lock poisoned".to_string(),
            })?;
        resources.insert(path_string, data);
        Ok(())
    }

    /// Remove a resource from the store.
    ///
    /// Returns `None` if the lock is poisoned or the resource doesn't exist.
    pub fn remove(&self, path: &str) -> Option<SharedResourceData> {
        self.resources.write().ok()?.remove(path)
    }

    /// Clear all resources from the store.
    ///
    /// Does nothing if the lock is poisoned.
    pub fn clear(&self) {
        if let Ok(mut resources) = self.resources.write() {
            resources.clear();
        }
    }

    /// Get the number of resources in the store.
    ///
    /// Returns 0 if the lock is poisoned.
    pub fn len(&self) -> usize {
        self.resources.read().map(|r| r.len()).unwrap_or(0)
    }

    /// Check if the store is empty.
    ///
    /// Returns `true` if the lock is poisoned (safe default).
    pub fn is_empty(&self) -> bool {
        self.resources.read().map(|r| r.is_empty()).unwrap_or(true)
    }
}

impl ResourceProvider for InMemoryResourceProvider {
    fn load(&self, path: &str) -> Result<SharedResourceData, ResourceError> {
        let resources = self
            .resources
            .read()
            .map_err(|_| ResourceError::LoadFailed {
                path: path.to_string(),
                message: "resource store lock poisoned".to_string(),
            })?;
        resources
            .get(path)
            .cloned()
            .ok_or_else(|| ResourceError::NotFound(path.to_string()))
    }

    fn exists(&self, path: &str) -> bool {
        self.resources
            .read()
            .map(|r| r.contains_key(path))
            .unwrap_or(false)
    }

    fn name(&self) -> &'static str {
        "InMemoryResourceProvider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_provider_add_and_load() {
        let provider = InMemoryResourceProvider::new();
        provider.add("test.txt", b"Hello, World!".to_vec()).unwrap();

        let data = provider.load("test.txt").unwrap();
        assert_eq!(&*data, b"Hello, World!");
    }

    #[test]
    fn test_in_memory_provider_not_found() {
        let provider = InMemoryResourceProvider::new();
        let result = provider.load("nonexistent.txt");
        assert!(matches!(result, Err(ResourceError::NotFound(_))));
    }

    #[test]
    fn test_in_memory_provider_exists() {
        let provider = InMemoryResourceProvider::new();
        provider.add("exists.txt", vec![]).unwrap();

        assert!(provider.exists("exists.txt"));
        assert!(!provider.exists("not_exists.txt"));
    }

    #[test]
    fn test_in_memory_provider_remove() {
        let provider = InMemoryResourceProvider::new();
        provider.add("test.txt", b"data".to_vec()).unwrap();

        assert!(provider.exists("test.txt"));
        provider.remove("test.txt");
        assert!(!provider.exists("test.txt"));
    }

    #[test]
    fn test_in_memory_provider_clear() {
        let provider = InMemoryResourceProvider::new();
        provider.add("a.txt", vec![]).unwrap();
        provider.add("b.txt", vec![]).unwrap();

        assert_eq!(provider.len(), 2);
        provider.clear();
        assert!(provider.is_empty());
    }

    // Edge case tests

    #[test]
    fn test_in_memory_provider_empty() {
        let provider = InMemoryResourceProvider::new();
        assert!(provider.is_empty());
        assert_eq!(provider.len(), 0);
    }

    #[test]
    fn test_in_memory_provider_overwrite() {
        let provider = InMemoryResourceProvider::new();
        provider.add("test.txt", b"original".to_vec()).unwrap();
        provider.add("test.txt", b"updated".to_vec()).unwrap();

        let data = provider.load("test.txt").unwrap();
        assert_eq!(&*data, b"updated");
        assert_eq!(provider.len(), 1);
    }

    #[test]
    fn test_in_memory_provider_empty_data() {
        let provider = InMemoryResourceProvider::new();
        provider.add("empty.bin", vec![]).unwrap();

        assert!(provider.exists("empty.bin"));
        let data = provider.load("empty.bin").unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn test_in_memory_provider_large_data() {
        let provider = InMemoryResourceProvider::new();
        let large_data = vec![0u8; 1_000_000]; // 1MB
        provider.add("large.bin", large_data.clone()).unwrap();

        let loaded = provider.load("large.bin").unwrap();
        assert_eq!(loaded.len(), 1_000_000);
        assert_eq!(&*loaded, &large_data);
    }

    #[test]
    fn test_in_memory_provider_add_shared() {
        let provider = InMemoryResourceProvider::new();
        let shared_data = Arc::new(vec![1, 2, 3, 4, 5]);
        provider
            .add_shared("shared.bin", shared_data.clone())
            .unwrap();

        let loaded = provider.load("shared.bin").unwrap();
        assert_eq!(&*loaded, &*shared_data);
    }

    #[test]
    fn test_in_memory_provider_remove_nonexistent() {
        let provider = InMemoryResourceProvider::new();
        // Should not panic when removing non-existent resource
        let result = provider.remove("does_not_exist.txt");
        assert!(result.is_none());
    }

    #[test]
    fn test_in_memory_provider_remove_returns_data() {
        let provider = InMemoryResourceProvider::new();
        provider.add("test.txt", b"data".to_vec()).unwrap();

        let removed = provider.remove("test.txt");
        assert!(removed.is_some());
        assert_eq!(&*removed.unwrap(), b"data");
        assert!(!provider.exists("test.txt"));
    }

    #[test]
    fn test_in_memory_provider_name() {
        let provider = InMemoryResourceProvider::new();
        assert_eq!(provider.name(), "InMemoryResourceProvider");
    }

    #[test]
    fn test_in_memory_provider_base_path() {
        let provider = InMemoryResourceProvider::new();
        // InMemoryResourceProvider has no base path
        assert!(provider.base_path().is_none());
    }

    #[test]
    fn test_resource_error_display() {
        let err = ResourceError::NotFound("test.txt".to_string());
        assert!(err.to_string().contains("test.txt"));

        let err = ResourceError::LoadFailed {
            path: "file.bin".to_string(),
            message: "permission denied".to_string(),
        };
        assert!(err.to_string().contains("file.bin"));
        assert!(err.to_string().contains("permission denied"));

        let err = ResourceError::InvalidFormat("corrupted data".to_string());
        assert!(err.to_string().contains("corrupted data"));
    }

    #[test]
    fn test_resource_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let resource_err: ResourceError = io_err.into();
        assert!(matches!(resource_err, ResourceError::Io(_)));
        assert!(resource_err.to_string().contains("file not found"));
    }
}
