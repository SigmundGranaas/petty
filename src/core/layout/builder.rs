//! Defines the trait and registry for constructing `LayoutNode`s from `IRNode`s.
//! This decoupling allows new node types to be registered without modifying the core engine.

use crate::core::idf::IRNode;
use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::node::RenderNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::collections::HashMap;
use std::sync::Arc;

/// A trait for types that can build a `RenderNode` from an `IRNode`.
pub trait NodeBuilder: Send + Sync {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError>;
}

/// A registry for mapping `IRNode` types (via `kind()`) to `NodeBuilder`s.
pub struct NodeRegistry {
    builders: HashMap<String, Box<dyn NodeBuilder>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            builders: HashMap::new(),
        }
    }

    pub fn register(&mut self, kind: &str, builder: Box<dyn NodeBuilder>) {
        self.builders.insert(kind.to_string(), builder);
    }

    pub fn get(&self, kind: &str) -> Option<&dyn NodeBuilder> {
        self.builders.get(kind).map(|b| b.as_ref())
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}