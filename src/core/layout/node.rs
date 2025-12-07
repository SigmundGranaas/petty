// src/core/layout/node.rs

use crate::core::idf::TextStr;
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::{ComputedStyle, LayoutEngine, LayoutError, PositionedElement};
use bumpalo::Bump;
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Instant;

// Concrete node implementations used in the RenderNode enum
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::nodes::flex::FlexNode;
use crate::core::layout::nodes::heading::HeadingNode;
use crate::core::layout::nodes::image::ImageNode;
use crate::core::layout::nodes::index_marker::IndexMarkerNode;
use crate::core::layout::nodes::list::ListNode;
use crate::core::layout::nodes::list_item::ListItemNode;
use crate::core::layout::nodes::page_break::PageBreakNode;
use crate::core::layout::nodes::paragraph::ParagraphNode;
use crate::core::layout::nodes::table::TableNode;

// --- State Definitions (Type-Safe) ---

#[derive(Debug, Clone)]
pub struct BlockState {
    pub child_index: usize,
    pub child_state: Option<Box<NodeState>>,
}

#[derive(Debug, Clone)]
pub struct FlexState {
    pub child_index: usize,
    pub child_state: Option<Box<NodeState>>,
}

#[derive(Debug, Clone)]
pub struct ListItemState {
    pub child_index: usize,
    pub child_state: Option<Box<NodeState>>,
}

#[derive(Debug, Clone)]
pub struct ParagraphState {
    pub scroll_offset: f32,
}

#[derive(Debug, Clone)]
pub struct TableState {
    pub row_index: usize,
}

/// Represents the resumption state for any node type.
#[derive(Debug, Clone)]
pub enum NodeState {
    Block(BlockState),
    Flex(FlexState),
    ListItem(ListItemState),
    Paragraph(ParagraphState),
    Table(TableState),
    // For nodes that break cleanly without internal data (e.g. PageBreak, Image)
    Atomic,
}

impl NodeState {
    pub fn as_block(self) -> Result<BlockState, LayoutError> {
        match self {
            NodeState::Block(s) => Ok(s),
            _ => Err(LayoutError::StateMismatch("Block", self.variant_name())),
        }
    }

    pub fn as_flex(self) -> Result<FlexState, LayoutError> {
        match self {
            NodeState::Flex(s) => Ok(s),
            _ => Err(LayoutError::StateMismatch("Flex", self.variant_name())),
        }
    }

    pub fn as_list_item(self) -> Result<ListItemState, LayoutError> {
        match self {
            NodeState::ListItem(s) => Ok(s),
            _ => Err(LayoutError::StateMismatch("ListItem", self.variant_name())),
        }
    }

    pub fn as_paragraph(self) -> Result<ParagraphState, LayoutError> {
        match self {
            NodeState::Paragraph(s) => Ok(s),
            _ => Err(LayoutError::StateMismatch("Paragraph", self.variant_name())),
        }
    }

    pub fn as_table(self) -> Result<TableState, LayoutError> {
        match self {
            NodeState::Table(s) => Ok(s),
            _ => Err(LayoutError::StateMismatch("Table", self.variant_name())),
        }
    }

    fn variant_name(&self) -> &'static str {
        match self {
            NodeState::Block(_) => "Block",
            NodeState::Flex(_) => "Flex",
            NodeState::ListItem(_) => "ListItem",
            NodeState::Paragraph(_) => "Paragraph",
            NodeState::Table(_) => "Table",
            NodeState::Atomic => "Atomic",
        }
    }
}

// --- Context and Environment ---

#[derive(Debug, Clone)]
pub struct AnchorLocation {
    pub local_page_index: usize,
    pub y_pos: f32,
}

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub local_page_index: usize,
    pub y_pos: f32,
}

/// Read-only (mostly) environment data shared across the layout pass.
/// It contains the `LayoutEngine`, page info, and a mutable cache for expensive computations.
pub struct LayoutEnvironment<'a> {
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,
    /// A cache for transient layout data (e.g. shaped text Buffers, Taffy trees).
    /// This allows `measure` and `layout` steps to share expensive results.
    pub cache: &'a mut HashMap<u64, Box<dyn Any + Send>>,
}

/// The mutable context passed down the tree during layout.
pub struct LayoutContext<'a> {
    // Composition: Wraps the environment to avoid field duplication and borrow conflicts.
    pub env: LayoutEnvironment<'a>,

    /// The Arena allocator for creating transient layout nodes/data.
    pub arena: &'a Bump,

    // Geometry
    bounds: geom::Rect,
    cursor: (f32, f32), // (x, y) relative to bounds

    // Outputs
    elements: &'a mut Vec<PositionedElement>,
    defined_anchors: &'a mut HashMap<TextStr, AnchorLocation>,
    index_entries: &'a mut HashMap<TextStr, Vec<IndexEntry>>,

    /// Tracks margin collapsing context between blocks
    pub last_v_margin: f32,
}

impl<'a> LayoutContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        env: LayoutEnvironment<'a>,
        bounds: geom::Rect,
        arena: &'a Bump,
        elements: &'a mut Vec<PositionedElement>,
        defined_anchors: &'a mut HashMap<TextStr, AnchorLocation>,
        index_entries: &'a mut HashMap<TextStr, Vec<IndexEntry>>,
    ) -> Self {
        Self {
            env,
            arena,
            bounds,
            cursor: (0.0, 0.0),
            elements,
            defined_anchors,
            index_entries,
            last_v_margin: 0.0,
        }
    }

    pub fn cursor_y(&self) -> f32 {
        self.cursor.1
    }

    pub fn set_cursor_y(&mut self, y: f32) {
        self.cursor.1 = y;
    }

    pub fn bounds(&self) -> geom::Rect {
        self.bounds
    }

    pub fn advance_cursor(&mut self, dy: f32) {
        self.cursor.1 += dy;
    }

    pub fn available_height(&self) -> f32 {
        (self.bounds.height - self.cursor.1).max(0.0)
    }

    pub fn register_anchor(&mut self, id: &str) {
        let location = AnchorLocation {
            local_page_index: self.env.local_page_index,
            y_pos: self.cursor.1 + self.bounds.y,
        };
        self.defined_anchors.insert(id.into(), location);
    }

    pub fn register_index_entry(&mut self, term: &str) {
        let entry = IndexEntry {
            local_page_index: self.env.local_page_index,
            y_pos: self.cursor.1 + self.bounds.y,
        };
        self.index_entries
            .entry(term.into())
            .or_default()
            .push(entry);
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

    /// Creates a new context for a child node with the specified bounds.
    /// Note: This re-borrows the mutable environment, splitting the borrow.
    pub fn child<'child>(&'child mut self, bounds: geom::Rect) -> LayoutContext<'child> {
        // We need to re-construct the LayoutEnvironment with a sub-borrow of the cache
        // However, `self.env` contains `&mut HashMap`. We can't re-borrow it mutably *easily*
        // if we just pass `self.env`.
        // BUT, `LayoutContext` owns `env` which is `LayoutEnvironment<'a>`.
        // `&'child mut self` allows us to borrow fields.

        let sub_env = LayoutEnvironment {
            engine: self.env.engine,
            local_page_index: self.env.local_page_index,
            cache: self.env.cache, // re-borrow mutable ref
        };

        LayoutContext {
            env: sub_env,
            arena: self.arena,
            bounds,
            cursor: (0.0, 0.0),
            elements: &mut *self.elements,
            defined_anchors: &mut *self.defined_anchors,
            index_entries: &mut *self.index_entries,
            last_v_margin: 0.0,
        }
    }
}

pub enum LayoutResult {
    Finished,
    Break(NodeState),
}

/// The core enum wrapping all possible layout nodes.
/// Uses references allocated in a bump arena (`&'a Node`) for performance.
#[derive(Debug, Clone, Copy)]
pub enum RenderNode<'a> {
    Block(&'a BlockNode<'a>),
    Flex(&'a FlexNode<'a>),
    Heading(&'a HeadingNode<'a>),
    Image(&'a ImageNode),
    IndexMarker(&'a IndexMarkerNode),
    List(&'a ListNode<'a>),
    ListItem(&'a ListItemNode<'a>),
    PageBreak(&'a PageBreakNode),
    Paragraph(&'a ParagraphNode<'a>),
    Table(&'a TableNode<'a>),
}

impl<'a> RenderNode<'a> {
    pub fn kind_str(&self) -> &'static str {
        match self {
            RenderNode::Block(_) => "Block",
            RenderNode::Flex(_) => "Flex",
            RenderNode::Heading(_) => "Heading",
            RenderNode::Image(_) => "Image",
            RenderNode::IndexMarker(_) => "IndexMarker",
            RenderNode::List(_) => "List",
            RenderNode::ListItem(_) => "ListItem",
            RenderNode::PageBreak(_) => "PageBreak",
            RenderNode::Paragraph(_) => "Paragraph",
            RenderNode::Table(_) => "Table",
        }
    }
}

// Explicit Static Dispatch Implementation with Performance Logging
impl<'a> LayoutNode for RenderNode<'a> {
    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let start = Instant::now();
        let kind = self.kind_str();

        let size = match self {
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
        };

        let duration = start.elapsed();
        env.engine.record_perf(&format!("Measure: {}", kind), duration);

        size
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        let start = Instant::now();
        let kind = self.kind_str();

        let res = match self {
            RenderNode::Block(n) => n.layout(ctx, constraints, break_state),
            RenderNode::Flex(n) => n.layout(ctx, constraints, break_state),
            RenderNode::Heading(n) => n.layout(ctx, constraints, break_state),
            RenderNode::Image(n) => n.layout(ctx, constraints, break_state),
            RenderNode::IndexMarker(n) => n.layout(ctx, constraints, break_state),
            RenderNode::List(n) => n.layout(ctx, constraints, break_state),
            RenderNode::ListItem(n) => n.layout(ctx, constraints, break_state),
            RenderNode::PageBreak(n) => n.layout(ctx, constraints, break_state),
            RenderNode::Paragraph(n) => n.layout(ctx, constraints, break_state),
            RenderNode::Table(n) => n.layout(ctx, constraints, break_state),
        };

        let duration = start.elapsed();
        ctx.env.engine.record_perf(&format!("Layout: {}", kind), duration);

        res
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

    fn check_for_page_break(&self) -> Option<Option<TextStr>> {
        match self {
            RenderNode::PageBreak(n) => n.check_for_page_break(),
            RenderNode::Block(n) => n.check_for_page_break(),
            _ => None,
        }
    }
}

pub trait LayoutNode: Debug + Sync {
    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size;

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError>;

    fn style(&self) -> &Arc<ComputedStyle>;

    fn check_for_page_break(&self) -> Option<Option<TextStr>> {
        None
    }
}