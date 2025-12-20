//! Data source abstractions for the pipeline.
//!
//! This module provides the `DataSource` trait and various implementations
//! for feeding data into the document generation pipeline.
//!
//! ## Available Sources
//!
//! - `IteratorDataSource`: Wraps any iterator
//! - `VecDataSource`: In-memory vector of data
//! - `ChannelDataSource`: Async channel receiver (feature-gated)
//!
//! ## Example
//!
//! ```ignore
//! use petty::source::{DataSource, VecDataSource};
//! use serde_json::json;
//!
//! let data = vec![json!({"id": 1}), json!({"id": 2})];
//! let mut source = VecDataSource::new(data);
//!
//! while let Some(item) = source.next() {
//!     println!("Processing: {:?}", item);
//! }
//! ```

use serde_json::Value;

/// A trait for data sources that feed items into the pipeline.
///
/// This abstraction allows the pipeline to work with various data sources:
/// - In-memory collections
/// - Iterators
/// - Async channels
/// - Database cursors
/// - Message queues (Kafka, RabbitMQ, etc.)
///
/// The trait is designed to be simple and flexible, supporting both
/// synchronous and asynchronous data sources.
pub trait DataSource: Send {
    /// Get the next data item, if available.
    ///
    /// Returns `None` when the source is exhausted.
    fn next(&mut self) -> Option<Value>;

    /// Hint about the total number of items (for progress reporting).
    ///
    /// Returns `None` if the size is unknown or unbounded.
    fn size_hint(&self) -> Option<usize> {
        None
    }

    /// Check if the source has a known size.
    fn has_known_size(&self) -> bool {
        self.size_hint().is_some()
    }
}

/// Wraps any iterator as a DataSource.
pub struct IteratorDataSource<I>
where
    I: Iterator<Item = Value> + Send,
{
    inner: I,
    size_hint: Option<usize>,
}

impl<I> IteratorDataSource<I>
where
    I: Iterator<Item = Value> + Send,
{
    /// Create a new data source from an iterator.
    pub fn new(inner: I) -> Self {
        let (lower, upper) = inner.size_hint();
        let size_hint = if upper == Some(lower) {
            Some(lower)
        } else {
            None
        };
        Self { inner, size_hint }
    }
}

impl<I> DataSource for IteratorDataSource<I>
where
    I: Iterator<Item = Value> + Send,
{
    fn next(&mut self) -> Option<Value> {
        self.inner.next()
    }

    fn size_hint(&self) -> Option<usize> {
        self.size_hint
    }
}

/// A data source backed by an in-memory vector.
///
/// This is the simplest data source, useful for small datasets or testing.
pub struct VecDataSource {
    data: Vec<Value>,
    index: usize,
}

impl VecDataSource {
    /// Create a new data source from a vector.
    pub fn new(data: Vec<Value>) -> Self {
        Self { data, index: 0 }
    }

    /// Get the total number of items.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the source is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the number of items remaining.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.index
    }
}

impl DataSource for VecDataSource {
    fn next(&mut self) -> Option<Value> {
        if self.index < self.data.len() {
            let item = self.data[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.data.len())
    }
}

// Blanket implementation for Box<dyn DataSource>
impl DataSource for Box<dyn DataSource> {
    fn next(&mut self) -> Option<Value> {
        (**self).next()
    }

    fn size_hint(&self) -> Option<usize> {
        (**self).size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_vec_data_source() {
        let data = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];
        let mut source = VecDataSource::new(data);

        assert_eq!(source.len(), 3);
        assert_eq!(source.remaining(), 3);
        assert_eq!(source.size_hint(), Some(3));
        assert!(source.has_known_size());

        assert_eq!(source.next(), Some(json!({"id": 1})));
        assert_eq!(source.remaining(), 2);

        assert_eq!(source.next(), Some(json!({"id": 2})));
        assert_eq!(source.remaining(), 1);

        assert_eq!(source.next(), Some(json!({"id": 3})));
        assert_eq!(source.remaining(), 0);

        assert_eq!(source.next(), None);
    }

    #[test]
    fn test_iterator_data_source() {
        let data = vec![json!(1), json!(2), json!(3)];
        let mut source = IteratorDataSource::new(data.into_iter());

        assert_eq!(source.size_hint(), Some(3));

        assert_eq!(source.next(), Some(json!(1)));
        assert_eq!(source.next(), Some(json!(2)));
        assert_eq!(source.next(), Some(json!(3)));
        assert_eq!(source.next(), None);
    }

    #[test]
    fn test_empty_vec_source() {
        let mut source = VecDataSource::new(vec![]);

        assert!(source.is_empty());
        assert_eq!(source.len(), 0);
        assert_eq!(source.remaining(), 0);
        assert_eq!(source.next(), None);
    }

    #[test]
    fn test_boxed_data_source() {
        let data = vec![json!(1), json!(2)];
        let mut source: Box<dyn DataSource> = Box::new(VecDataSource::new(data));

        assert_eq!(source.size_hint(), Some(2));
        assert_eq!(source.next(), Some(json!(1)));
        assert_eq!(source.next(), Some(json!(2)));
        assert_eq!(source.next(), None);
    }
}
