use super::{geom, ComputedStyle, LayoutEngine, LayoutError, PositionedElement};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

/// Stores the resolved location of an anchor target.
#[derive(Debug, Clone)]
pub struct AnchorLocation {
    /// The 0-based index of the page within the current sequence.
    pub local_page_index: usize,
    /// The Y position of the anchor on that page, in points.
    pub y_pos: f32,
}

/// The canvas given by a parent to a child for a single layout operation.
///
/// This struct holds all the contextual information a `LayoutNode` needs to
/// perform its work, including the available space, the current pen position (cursor),
/// and a reference to the list of elements being generated for the current page.
pub struct LayoutContext<'a> {
    /// A reference to the layout engine, providing access to font metrics and styling.
    pub engine: &'a LayoutEngine,
    /// The absolute position and size of the canvas for this layout operation.
    pub bounds: geom::Rect,
    /// The current "pen position" on the canvas, relative to `bounds.x` and `bounds.y`.
    pub cursor: (f32, f32),
    /// The collection of drawable elements being generated for the current page.
    pub elements: &'a RefCell<Vec<PositionedElement>>,
    /// The bottom margin of the previously laid-out block-level sibling, for margin collapsing.
    pub last_v_margin: f32,
    /// The 0-based index of the current page being laid out.
    pub local_page_index: usize,
    /// A mutable collection of all anchors defined in the document.
    pub defined_anchors: &'a RefCell<HashMap<String, AnchorLocation>>,
}

impl<'a> LayoutContext<'a> {
    /// Creates a new layout context for a page.
    pub fn new(
        engine: &'a LayoutEngine,
        bounds: geom::Rect,
        elements: &'a RefCell<Vec<PositionedElement>>,
        defined_anchors: &'a RefCell<HashMap<String, AnchorLocation>>,
    ) -> Self {
        Self {
            engine,
            bounds,
            cursor: (0.0, 0.0),
            elements,
            last_v_margin: 0.0,
            local_page_index: 0,
            defined_anchors,
        }
    }

    /// Advances the vertical cursor by the given amount.
    pub fn advance_cursor(&mut self, y_amount: f32) {
        self.cursor.1 += y_amount;
    }

    /// Returns the remaining vertical space available in this context.
    pub fn available_height(&self) -> f32 {
        (self.bounds.height - self.cursor.1).max(0.0)
    }

    /// Pushes a drawable element onto the current page at the current cursor position.
    ///
    /// The provided element's coordinates are treated as relative to the context's
    /// current cursor position. This method translates them into absolute page
    /// coordinates before adding the element to the page.
    pub fn push_element(&mut self, element: PositionedElement) {
        let mut final_element = element;
        final_element.x += self.bounds.x + self.cursor.0;
        final_element.y += self.bounds.y + self.cursor.1;
        self.elements.borrow_mut().push(final_element);
    }

    /// Pushes a drawable element at a specific coordinate relative to the context's origin.
    ///
    /// This is useful for drawing backgrounds or other elements that are not part of
    /// the normal vertical flow. Coordinates are relative to `bounds.x` and `bounds.y`.
    pub fn push_element_at(&mut self, mut element: PositionedElement, x: f32, y: f32) {
        element.x += self.bounds.x + x;
        element.y += self.bounds.y + y;
        self.elements.borrow_mut().push(element);
    }

    /// Checks if any elements have been added to the current page.
    pub fn is_empty(&self) -> bool {
        self.elements.borrow().is_empty()
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
///
/// Every layoutable element (block, paragraph, table) will have a corresponding
/// struct that implements this trait. The design enables a single, cooperative,
/// stateful layout pass that handles pagination naturally.
pub trait LayoutNode: Debug + Send + Sync + CloneLayoutNode {
    /// Lays out the node within the given context.
    ///
    /// It is the node's responsibility to:
    /// 1. Check if it fits in the remaining space of the context.
    /// 2. If it fits, push its drawable elements to the context and advance the cursor.
    /// 3. If it does not fit (or only partially fits), lay out what it can,
    ///    and return the "remainder" of itself as `LayoutResult::Partial`.
    /// 4. If the content is fundamentally too large for a page, return `Err(LayoutError)`.
    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError>;

    /// An optional pre-pass to calculate size-dependent properties like table columns.
    /// This method is responsible for storing its results within its own struct.
    fn measure(&mut self, _engine: &LayoutEngine, _available_width: f32) {
        // Default implementation does nothing.
    }

    /// Calculates the total vertical space the node would occupy if rendered with
    /// the given width constraint.
    ///
    /// This is a critical part of the `measure` pass for container elements, allowing
    /// them to determine children's sizes without performing a full layout.
    fn measure_content_height(&mut self, _engine: &LayoutEngine, _available_width: f32) -> f32 {
        // A default implementation for simple, non-wrapping elements might just return a fixed height.
        // Containers will need to override this to sum their children's heights.
        0.0
    }

    /// Measures the intrinsic "max-content" width of the node.
    /// This is used for `flex-basis: auto` calculation where an element has no explicit width.
    fn measure_intrinsic_width(&self, _engine: &LayoutEngine) -> f32 {
        // Default implementation for nodes that don't have an obvious intrinsic width (e.g., Block).
        // They will rely on explicit width/flex-basis or flex-grow properties.
        0.0
    }

    /// Returns the computed style of the node.
    fn style(&self) -> &Arc<ComputedStyle>;

    /// Checks if this node (or its first child) is an explicit page break.
    /// If it is, it consumes the page break node and returns the new master name.
    /// This allows the layout engine to react to page breaks discovered during pagination.
    fn check_for_page_break(&mut self) -> Option<Option<String>> {
        None
    }

    // Required for downcasting
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