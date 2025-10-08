// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/page_break.rs

use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::any::Any;
use std::sync::Arc;

/// A special `LayoutNode` that represents an explicit page break.
/// Its primary purpose is to act as a marker during the layout process.
#[derive(Debug, Clone)]
pub struct PageBreakNode {
    pub master_name: Option<String>,
    style: Arc<ComputedStyle>, // Needs a style to satisfy the trait
}

impl PageBreakNode {
    pub fn new(master_name: Option<String>) -> Self {
        Self {
            master_name,
            style: Arc::new(ComputedStyle::default()),
        }
    }
}

impl LayoutNode for PageBreakNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        // A page break should force a new page if it's not at the very top.
        if !ctx.is_empty() || ctx.cursor.1 > 0.0 {
            // By returning Partial with ourselves as the remainder, we signal to the
            // layout engine that a break is needed.
            Ok(LayoutResult::Partial(Box::new(self.clone())))
        } else {
            // If we are at the top of a page, we do nothing and are consumed.
            Ok(LayoutResult::Full)
        }
    }
}