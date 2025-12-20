use crate::{ComputedStyle, LayoutEngine, LayoutError, PositionedElement};
use bumpalo::Bump;
use petty_idf::TextStr;
use petty_types::geometry::{self, BoxConstraints, Size};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;

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

#[derive(Debug, Clone)]
pub enum NodeState {
    Block(BlockState),
    Flex(FlexState),
    ListItem(ListItemState),
    Paragraph(ParagraphState),
    Table(TableState),
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
pub struct LayoutEnvironment<'a> {
    pub engine: &'a LayoutEngine,
    pub local_page_index: usize,
    /// A cache for transient layout data (e.g. shaped text Buffers, Taffy trees).
    pub cache: &'a RefCell<HashMap<u64, Box<dyn Any + Send>>>,
}

pub struct LayoutContext<'a> {
    pub env: LayoutEnvironment<'a>,
    pub arena: &'a Bump,
    bounds: geometry::Rect,
    cursor: (f32, f32),
    elements: &'a mut Vec<PositionedElement>,
    defined_anchors: &'a mut HashMap<TextStr, AnchorLocation>,
    index_entries: &'a mut HashMap<TextStr, Vec<IndexEntry>>,
    pub last_v_margin: f32,
    root_top_y: f32,
}

impl<'a> LayoutContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        env: LayoutEnvironment<'a>,
        bounds: geometry::Rect,
        arena: &'a Bump,
        elements: &'a mut Vec<PositionedElement>,
        defined_anchors: &'a mut HashMap<TextStr, AnchorLocation>,
        index_entries: &'a mut HashMap<TextStr, Vec<IndexEntry>>,
    ) -> Self {
        let root_top_y = bounds.y;
        Self {
            env,
            arena,
            bounds,
            cursor: (0.0, 0.0),
            elements,
            defined_anchors,
            index_entries,
            last_v_margin: 0.0,
            root_top_y,
        }
    }

    pub fn cursor_y(&self) -> f32 {
        self.cursor.1
    }

    pub fn set_cursor_y(&mut self, y: f32) {
        self.cursor.1 = y;
    }

    pub fn is_at_page_top(&self) -> bool {
        let absolute_y = self.bounds.y + self.cursor.1;
        (absolute_y - self.root_top_y).abs() < 0.1
    }

    pub fn bounds(&self) -> geometry::Rect {
        self.bounds
    }

    pub fn advance_cursor(&mut self, dy: f32) {
        self.cursor.1 += dy;
    }

    pub fn available_height(&self) -> f32 {
        (self.bounds.height - self.cursor.1).max(0.0)
    }

    pub fn prepare_for_block(&mut self, top_margin: f32) -> bool {
        let margin_to_add = top_margin.max(self.last_v_margin);
        if self.cursor_y() > 0.001 && margin_to_add > self.available_height() {
            return true;
        }
        self.advance_cursor(margin_to_add);
        self.last_v_margin = 0.0;
        false
    }

    pub fn finish_block(&mut self, bottom_margin: f32) {
        self.last_v_margin = bottom_margin;
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

    pub fn child<'child>(&'child mut self, bounds: geometry::Rect) -> LayoutContext<'child> {
        let sub_env = LayoutEnvironment {
            engine: self.env.engine,
            local_page_index: self.env.local_page_index,
            cache: self.env.cache,
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
            root_top_y: self.root_top_y,
        }
    }
}

// Ensure LayoutResult is public and available
#[derive(Debug, Clone)]
pub enum LayoutResult {
    Finished,
    Break(NodeState),
}

pub trait LayoutNode: Debug + Sync {
    fn measure(
        &self,
        env: &LayoutEnvironment,
        constraints: BoxConstraints,
    ) -> Result<Size, LayoutError>;

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError>;

    fn style(&self) -> &ComputedStyle;

    fn check_for_page_break(&self) -> Option<Option<TextStr>> {
        None
    }
}
