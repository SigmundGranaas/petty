#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConfig {
    /// The maximum number of layout items (sequences) to process before the layout engine's
    /// internal caches (e.g., text shaping results) are pruned or reset.
    ///
    /// - **Higher values**: Better performance (CPU) as common glyphs/words are cached,
    ///   but higher memory usage.
    /// - **Lower values**: Lower memory usage, but potentially higher CPU usage due to
    ///   re-shaping text.
    ///
    /// Defaults to `10000`.
    pub cache_capacity: usize,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            cache_capacity: 10000,
        }
    }
}
