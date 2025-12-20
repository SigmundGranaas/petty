//! Newtype wrappers for semantic IDs and URIs
//!
//! These types provide compile-time type safety to prevent mixing up
//! different kinds of string identifiers (anchor IDs, resource URIs, index terms, etc.).

use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

/// An identifier for a document anchor (e.g., for cross-references and links)
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AnchorId(Arc<str>);

impl AnchorId {
    /// Creates a new AnchorId from a string
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }

    /// Returns the string representation of this anchor ID
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AnchorId {
    fn from(s: String) -> Self {
        Self(s.into())
    }
}

impl From<&str> for AnchorId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<Arc<str>> for AnchorId {
    fn from(s: Arc<str>) -> Self {
        Self(s)
    }
}

impl AsRef<str> for AnchorId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AnchorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A URI for a document resource (images, fonts, etc.)
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ResourceUri(Arc<str>);

impl ResourceUri {
    /// Creates a new ResourceUri from a string
    pub fn new(uri: impl Into<Arc<str>>) -> Self {
        Self(uri.into())
    }

    /// Returns the string representation of this resource URI
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ResourceUri {
    fn from(s: String) -> Self {
        Self(s.into())
    }
}

impl From<&str> for ResourceUri {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<Arc<str>> for ResourceUri {
    fn from(s: Arc<str>) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ResourceUri {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A term in a document index
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IndexTerm(Arc<str>);

impl IndexTerm {
    /// Creates a new IndexTerm from a string
    pub fn new(term: impl Into<Arc<str>>) -> Self {
        Self(term.into())
    }

    /// Returns the string representation of this index term
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for IndexTerm {
    fn from(s: String) -> Self {
        Self(s.into())
    }
}

impl From<&str> for IndexTerm {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<Arc<str>> for IndexTerm {
    fn from(s: Arc<str>) -> Self {
        Self(s)
    }
}

impl AsRef<str> for IndexTerm {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IndexTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_id_creation() {
        let id1 = AnchorId::new("section-1");
        let id2 = AnchorId::from("section-1");
        let id3 = AnchorId::from(String::from("section-1"));

        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
        assert_eq!(id1.as_str(), "section-1");
    }

    #[test]
    fn test_resource_uri_creation() {
        let uri1 = ResourceUri::new("images/logo.png");
        let uri2 = ResourceUri::from("images/logo.png");

        assert_eq!(uri1, uri2);
        assert_eq!(uri1.as_str(), "images/logo.png");
    }

    #[test]
    fn test_index_term_creation() {
        let term1 = IndexTerm::new("API");
        let term2 = IndexTerm::from("API");

        assert_eq!(term1, term2);
        assert_eq!(term1.as_str(), "API");
    }

    #[test]
    fn test_type_safety() {
        // This demonstrates type safety - these are different types
        // even though they wrap the same underlying string
        let anchor = AnchorId::new("test");
        let resource = ResourceUri::new("test");

        // These are different types - this line would not compile:
        // let _: bool = anchor == resource;

        // But their string representations are the same
        assert_eq!(anchor.as_str(), resource.as_str());
    }

    #[test]
    fn test_hash_map_usage() {
        use std::collections::HashMap;

        let mut anchors = HashMap::new();
        anchors.insert(AnchorId::new("section-1"), 42);
        anchors.insert(AnchorId::new("section-2"), 100);

        assert_eq!(anchors.get(&AnchorId::new("section-1")), Some(&42));

        let mut resources = HashMap::new();
        resources.insert(ResourceUri::new("image.png"), vec![1, 2, 3]);

        assert_eq!(
            resources.get(&ResourceUri::new("image.png")),
            Some(&vec![1, 2, 3])
        );
    }
}
