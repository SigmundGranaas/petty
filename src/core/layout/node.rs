use crate::core::layout::geom::{BoxConstraints, Size};
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use crate::core::layout::{geom, ComputedStyle, LayoutEngine, LayoutError, PositionedElement};

/// Stores the resolved location of an anchor target.
#[derive(Debug, Clone)]
pub struct AnchorLocation {
    /// The 0-based index of the page within the current sequence.
    pub local_page_index: usize,
    /// The Y position of the anchor on that page, in points.
    pub y_pos: f32,
}

/// Stores the resolved location of an index term.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// The 0-based index of the page within the current sequence.
    pub local_page_index: usize,
    /// The Y position of the term on that page, in points.
    pub y_pos: f32,
}

/// The immutable environment (read-only access to fonts, styles, global config).
pub struct LayoutEnvironment<'a> {
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,
}

/// The mutable state (writing the output).
pub struct LayoutBuffer<'a> {
    pub bounds: geom::Rect,
    pub cursor: (f32, f32),
    pub elements: &'a mut Vec<PositionedElement>,
    pub last_v_margin: f32,
    pub defined_anchors: &'a mut HashMap<String, AnchorLocation>,
    pub index_entries: &'a mut HashMap<String, Vec<IndexEntry>>,
}

impl<'a> LayoutBuffer<'a> {
    pub fn new(
        bounds: geom::Rect,
        elements: &'a mut Vec<PositionedElement>,
        defined_anchors: &'a mut HashMap<String, AnchorLocation>,
        index_entries: &'a mut HashMap<String, Vec<IndexEntry>>,
    ) -> Self {
        Self {
            bounds,
            cursor: (0.0, 0.0),
            elements,
            last_v_margin: 0.0,
            defined_anchors,
            index_entries,
        }
    }

    pub fn advance_cursor(&mut self, y_amount: f32) {
        self.cursor.1 += y_amount;
    }

    pub fn available_height(&self) -> f32 {
        (self.bounds.height - self.cursor.1).max(0.0)
    }

    pub fn push_element(&mut self, element: PositionedElement) {
        let mut final_element = element;
        final_element.x += self.bounds.x + self.cursor.0;
        final_element.y += self.bounds.y + self.cursor.1;
        self.elements.push(final_element);
    }

    pub fn push_element_at(&mut self, mut element: PositionedElement, x: f32, y: f32) {
        element.x += self.bounds.x + x;
        element.y += self.bounds.y + y;
        self.elements.push(element);
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

/// The result of a layout operation, enabling cooperative page breaking.
#[derive(Debug)]
pub enum LayoutResult {
    /// The entire node was laid out successfully.
    Full,
    /// The node was partially laid out. The returned value is the
    /// remainder of the node that needs to be placed on the next page.
    Partial(Box<dyn LayoutNode>),
}

/// A helper trait for cloning trait objects.
pub trait CloneLayoutNode {
    fn clone_box(&self) -> Box<dyn LayoutNode>;
}

impl<T> CloneLayoutNode for T
where
    T: 'static + LayoutNode + Clone,
{
    fn clone_box(&self) -> Box<dyn LayoutNode> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn LayoutNode> {
    fn clone(&self) -> Box<dyn LayoutNode> {
        self.clone_box()
    }
}

/// The central trait that governs all layout.
pub trait LayoutNode: Debug + Send + Sync + CloneLayoutNode {
    /// Performs a measurement pass based on the given constraints.
    ///
    /// This method serves two purposes:
    /// 1. Determine the size of the node given the constraints.
    /// 2. Perform any expensive pre-calculations (like line breaking or table column sizing)
    ///    and store them in the node state for the subsequent `layout` pass.
    ///
    /// If `constraints` are unbounded (e.g. max_width is INFINITY), this method should return
    /// the node's "intrinsic" size.
    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size;

    /// Performs the actual layout, writing elements to the buffer.
    /// This must be called after `measure`.
    fn layout(&mut self, env: &LayoutEnvironment, buf: &mut LayoutBuffer) -> Result<LayoutResult, LayoutError>;

    fn style(&self) -> &Arc<ComputedStyle>;

    fn check_for_page_break(&mut self) -> Option<Option<String>> {
        None
    }

    fn as_any(&self) -> &dyn Any;
}

impl dyn LayoutNode {
    pub fn is<T: Any>(&self) -> bool {
        self.as_any().is::<T>()
    }
    pub fn downcast<T: Any>(self: Box<Self>) -> Result<Box<T>, Box<dyn LayoutNode>> {
        if self.is::<T>() {
            unsafe {
                let raw: *mut dyn LayoutNode = Box::into_raw(self);
                Ok(Box::from_raw(raw as *mut T))
            }
        } else {
            Err(self)
        }
    }
}