// src/core/layout/node.rs

use crate::core::idf::TextStr;
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::{ComputedStyle, LayoutEngine, LayoutError, PositionedElement};
use bumpalo::Bump;
use cosmic_text::FontSystem;
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

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

/// Read-only environment data shared across the layout pass.
pub struct LayoutEnvironment<'a, 'b> {
    pub engine: &'a LayoutEngine,
    pub font_system: &'b mut FontSystem,
    pub local_page_index: usize,
}

/// The mutable context passed down the tree during layout.
pub struct LayoutContext<'a, 'b> {
    pub engine: &'a LayoutEngine,
    pub font_system: &'b mut FontSystem,
    /// The Arena allocator for creating transient layout nodes/data.
    pub arena: &'a Bump,
    pub local_page_index: usize,

    // Geometry
    bounds: geom::Rect,
    cursor: (f32, f32), // (x, y) relative to bounds

    // Outputs
    elements: &'a mut Vec<PositionedElement>,
    defined_anchors: &'a mut HashMap<TextStr, AnchorLocation>,
    index_entries: &'a mut HashMap<TextStr, Vec<IndexEntry>>,

    /// Tracks margin collapsing context between blocks
    pub last_v_margin: f32,

    /// A cache for transient layout data (e.g. Taffy trees for Flex nodes)
    pub layout_cache: &'a mut HashMap<u64, Box<dyn Any + Send>>,
}

impl<'a, 'b> LayoutContext<'a, 'b> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        env: LayoutEnvironment<'a, 'b>,
        bounds: geom::Rect,
        arena: &'a Bump,
        elements: &'a mut Vec<PositionedElement>,
        defined_anchors: &'a mut HashMap<TextStr, AnchorLocation>,
        index_entries: &'a mut HashMap<TextStr, Vec<IndexEntry>>,
        layout_cache: &'a mut HashMap<u64, Box<dyn Any + Send>>,
    ) -> Self {
        Self {
            engine: env.engine,
            font_system: env.font_system,
            local_page_index: env.local_page_index,
            arena,
            bounds,
            cursor: (0.0, 0.0),
            elements,
            defined_anchors,
            index_entries,
            last_v_margin: 0.0,
            layout_cache,
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
            local_page_index: self.local_page_index,
            y_pos: self.cursor.1 + self.bounds.y,
        };
        self.defined_anchors.insert(id.into(), location);
    }

    pub fn register_index_entry(&mut self, term: &str) {
        let entry = IndexEntry {
            local_page_index: self.local_page_index,
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

    pub fn with_child_bounds<R>(
        &mut self,
        child_bounds: geom::Rect,
        f: impl FnOnce(&mut LayoutContext) -> R,
    ) -> R {
        // Temporarily re-borrow font_system
        // Note: we must reconstruct context carefully.
        // Rust cannot reborrow mutable reference if it is already moved?
        // LayoutContext owns mutable references. We can reborrow fields.
        let mut child_ctx = LayoutContext {
            engine: self.engine,
            font_system: self.font_system,
            local_page_index: self.local_page_index,
            arena: self.arena,
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: self.elements,
            defined_anchors: self.defined_anchors,
            index_entries: self.index_entries,
            last_v_margin: 0.0,
            layout_cache: self.layout_cache,
        };
        f(&mut child_ctx)
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
    Paragraph(&'a ParagraphNode<'a>), // Added lifetime here
    Table(&'a TableNode<'a>),
}

impl<'a> LayoutNode for RenderNode<'a> {
    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
        match self {
            Self::Block(n) => n.measure(env, constraints),
            Self::Flex(n) => n.measure(env, constraints),
            Self::Heading(n) => n.measure(env, constraints),
            Self::Image(n) => n.measure(env, constraints),
            Self::IndexMarker(n) => n.measure(env, constraints),
            Self::List(n) => n.measure(env, constraints),
            Self::ListItem(n) => n.measure(env, constraints),
            Self::PageBreak(n) => n.measure(env, constraints),
            Self::Paragraph(n) => n.measure(env, constraints),
            Self::Table(n) => n.measure(env, constraints),
        }
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        match self {
            Self::Block(n) => n.layout(ctx, constraints, break_state),
            Self::Flex(n) => n.layout(ctx, constraints, break_state),
            Self::Heading(n) => n.layout(ctx, constraints, break_state),
            Self::Image(n) => n.layout(ctx, constraints, break_state),
            Self::IndexMarker(n) => n.layout(ctx, constraints, break_state),
            Self::List(n) => n.layout(ctx, constraints, break_state),
            Self::ListItem(n) => n.layout(ctx, constraints, break_state),
            Self::PageBreak(n) => n.layout(ctx, constraints, break_state),
            Self::Paragraph(n) => n.layout(ctx, constraints, break_state),
            Self::Table(n) => n.layout(ctx, constraints, break_state),
        }
    }

    fn style(&self) -> &Arc<ComputedStyle> {
        match self {
            Self::Block(n) => n.style(),
            Self::Flex(n) => n.style(),
            Self::Heading(n) => n.style(),
            Self::Image(n) => n.style(),
            Self::IndexMarker(n) => n.style(),
            Self::List(n) => n.style(),
            Self::ListItem(n) => n.style(),
            Self::PageBreak(n) => n.style(),
            Self::Paragraph(n) => n.style(),
            Self::Table(n) => n.style(),
        }
    }

    fn check_for_page_break(&self) -> Option<Option<TextStr>> {
        match self {
            Self::Block(n) => n.check_for_page_break(),
            Self::Flex(n) => n.check_for_page_break(),
            Self::Heading(n) => n.check_for_page_break(),
            Self::Image(n) => n.check_for_page_break(),
            Self::IndexMarker(n) => n.check_for_page_break(),
            Self::List(n) => n.check_for_page_break(),
            Self::ListItem(n) => n.check_for_page_break(),
            Self::PageBreak(n) => n.check_for_page_break(),
            Self::Paragraph(n) => n.check_for_page_break(),
            Self::Table(n) => n.check_for_page_break(),
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