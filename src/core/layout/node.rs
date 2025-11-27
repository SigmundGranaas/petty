use crate::core::layout::{geom, ComputedStyle, LayoutEngine, LayoutError, PositionedElement};
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use crate::core::layout::geom::{BoxConstraints, Size};

/// Stores the resolved location of an anchor target.
#[derive(Debug, Clone)]
pub struct AnchorLocation {
    pub local_page_index: usize,
    pub y_pos: f32,
}

/// Stores the resolved location of an index term.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub local_page_index: usize,
    pub y_pos: f32,
}

/// The immutable environment (read-only access to fonts, styles, global config).
#[derive(Clone, Copy)]
pub struct LayoutEnvironment<'a> {
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,
}

/// A unified context for the layout pass, handling cursor state, bounds, and output buffers.
pub struct LayoutContext<'a> {
    // Environment
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,

    // Mutable Output State
    pub bounds: geom::Rect,
    pub cursor: (f32, f32),
    pub elements: &'a mut Vec<PositionedElement>,
    pub defined_anchors: &'a mut HashMap<String, AnchorLocation>,
    pub index_entries: &'a mut HashMap<String, Vec<IndexEntry>>,
    pub last_v_margin: f32,
}

impl<'a> LayoutContext<'a> {
    pub fn new(
        env: LayoutEnvironment<'a>,
        bounds: geom::Rect,
        elements: &'a mut Vec<PositionedElement>,
        defined_anchors: &'a mut HashMap<String, AnchorLocation>,
        index_entries: &'a mut HashMap<String, Vec<IndexEntry>>,
    ) -> Self {
        Self {
            engine: env.engine,
            local_page_index: env.local_page_index,
            bounds,
            cursor: (0.0, 0.0),
            elements,
            defined_anchors,
            index_entries,
            last_v_margin: 0.0,
        }
    }

    pub fn advance_cursor(&mut self, y_amount: f32) {
        self.cursor.1 += y_amount;
    }

    pub fn available_height(&self) -> f32 {
        (self.bounds.height - self.cursor.1).max(0.0)
    }

    pub fn push_element(&mut self, mut element: PositionedElement) {
        element.x += self.bounds.x + self.cursor.0;
        element.y += self.bounds.y + self.cursor.1;
        self.elements.push(element);
    }

    pub fn push_element_at(&mut self, mut element: PositionedElement, x: f32, y: f32) {
        element.x += self.bounds.x + x;
        element.y += self.bounds.y + y;
        self.elements.push(element);
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn with_child_bounds<R>(
        &mut self,
        child_bounds: geom::Rect,
        f: impl FnOnce(&mut LayoutContext) -> R,
    ) -> R {
        let mut child_ctx = LayoutContext {
            engine: self.engine,
            local_page_index: self.local_page_index,
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: self.elements,
            defined_anchors: self.defined_anchors,
            index_entries: self.index_entries,
            last_v_margin: 0.0,
        };
        f(&mut child_ctx)
    }
}

/// The result of a layout operation, using state for pagination.
#[derive(Debug)]
pub enum LayoutResult {
    Finished,
    Break(Box<dyn Any + Send>),
}

/// A trait object representing a node in the render tree.
pub type RenderNode = Box<dyn LayoutNode>;

/// The central trait that governs all layout.
pub trait LayoutNode: Debug + Send + Sync {
    /// Performs a measurement pass based on the given constraints.
    /// This should be stateless and immutable.
    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size;

    /// Performs the actual layout.
    /// Takes an optional `break_state` to resume layout from a previous page break.
    /// Returns `LayoutResult::Break` with new state if content doesn't fit.
    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>,
    ) -> Result<LayoutResult, LayoutError>;

    fn style(&self) -> &Arc<ComputedStyle>;

    fn check_for_page_break(&self) -> Option<Option<String>> {
        None
    }
}