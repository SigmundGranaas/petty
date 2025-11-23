use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::{geom, ComputedStyle, LayoutEngine, LayoutError, PositionedElement};
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

// Import all specific node types to form the RenderNode enum
use crate::core::layout::nodes::{
    block::BlockNode,
    flex::FlexNode,
    heading::HeadingNode,
    image::ImageNode,
    index_marker::IndexMarkerNode,
    list::ListNode,
    list_item::ListItemNode,
    page_break::PageBreakNode,
    paragraph::ParagraphNode,
    table::TableNode,
};

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
#[derive(Clone, Copy)]
pub struct LayoutEnvironment<'a> {
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,
}

/// A unified context for the layout pass, handling cursor state, bounds, and output buffers.
pub struct LayoutContext<'a> {
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,
    /// The bounding box for the current element's content.
    pub bounds: geom::Rect,
    /// The current write cursor position relative to `bounds`.
    pub cursor: (f32, f32),
    pub elements: &'a mut Vec<PositionedElement>,
    pub last_v_margin: f32,
    pub defined_anchors: &'a mut HashMap<String, AnchorLocation>,
    pub index_entries: &'a mut HashMap<String, Vec<IndexEntry>>,
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

    /// Creates a child context for laying out nested content.
    /// This borrows mutable state from the parent context.
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
            last_v_margin: 0.0, // Child block contexts start fresh
            defined_anchors: self.defined_anchors,
            index_entries: self.index_entries,
        };

        f(&mut child_ctx)
    }
}

/// The result of a layout operation, enabling cooperative page breaking.
#[derive(Debug)]
pub enum LayoutResult {
    /// The entire node was laid out successfully.
    Full,
    /// The node was partially laid out. The returned value is the
    /// remainder of the node that needs to be placed on the next page.
    Partial(RenderNode),
}

/// The main enum that wraps all concrete layout nodes.
/// This replaces `Box<dyn LayoutNode>` to use static dispatch.
#[derive(Debug, Clone)]
pub enum RenderNode {
    Block(BlockNode),
    Flex(FlexNode),
    Heading(HeadingNode),
    Image(ImageNode),
    IndexMarker(IndexMarkerNode),
    List(ListNode),
    ListItem(ListItemNode),
    PageBreak(PageBreakNode),
    Paragraph(ParagraphNode),
    Table(TableNode),
}

impl RenderNode {
    // Helper to delegate `is` checks without casting
    pub fn is_page_break(&self) -> bool {
        matches!(self, RenderNode::PageBreak(_))
    }

    // Helper to extract specific types if needed
    pub fn as_page_break(&self) -> Option<&PageBreakNode> {
        if let RenderNode::PageBreak(n) = self { Some(n) } else { None }
    }
}

/// The central trait that governs all layout.
pub trait LayoutNode: Debug + Send + Sync + Any {
    /// Performs a measurement pass based on the given constraints.
    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size;

    /// Performs the actual layout, writing elements to the context.
    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError>;

    fn style(&self) -> &Arc<ComputedStyle>;

    fn check_for_page_break(&mut self) -> Option<Option<String>> {
        None
    }
}

// Implement LayoutNode for RenderNode via enum dispatch
impl LayoutNode for RenderNode {
    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        match self {
            RenderNode::Block(n) => n.measure(env, constraints),
            RenderNode::Flex(n) => n.measure(env, constraints),
            RenderNode::Heading(n) => n.measure(env, constraints),
            RenderNode::Image(n) => n.measure(env, constraints),
            RenderNode::IndexMarker(n) => n.measure(env, constraints),
            RenderNode::List(n) => n.measure(env, constraints),
            RenderNode::ListItem(n) => n.measure(env, constraints),
            RenderNode::PageBreak(n) => n.measure(env, constraints),
            RenderNode::Paragraph(n) => n.measure(env, constraints),
            RenderNode::Table(n) => n.measure(env, constraints),
        }
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        match self {
            RenderNode::Block(n) => n.layout(ctx),
            RenderNode::Flex(n) => n.layout(ctx),
            RenderNode::Heading(n) => n.layout(ctx),
            RenderNode::Image(n) => n.layout(ctx),
            RenderNode::IndexMarker(n) => n.layout(ctx),
            RenderNode::List(n) => n.layout(ctx),
            RenderNode::ListItem(n) => n.layout(ctx),
            RenderNode::PageBreak(n) => n.layout(ctx),
            RenderNode::Paragraph(n) => n.layout(ctx),
            RenderNode::Table(n) => n.layout(ctx),
        }
    }

    fn style(&self) -> &Arc<ComputedStyle> {
        match self {
            RenderNode::Block(n) => n.style(),
            RenderNode::Flex(n) => n.style(),
            RenderNode::Heading(n) => n.style(),
            RenderNode::Image(n) => n.style(),
            RenderNode::IndexMarker(n) => n.style(),
            RenderNode::List(n) => n.style(),
            RenderNode::ListItem(n) => n.style(),
            RenderNode::PageBreak(n) => n.style(),
            RenderNode::Paragraph(n) => n.style(),
            RenderNode::Table(n) => n.style(),
        }
    }

    fn check_for_page_break(&mut self) -> Option<Option<String>> {
        match self {
            RenderNode::Block(n) => n.check_for_page_break(),
            RenderNode::List(n) => n.check_for_page_break(),
            _ => None,
        }
    }
}