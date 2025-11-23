use crate::core::layout::node::{LayoutContext, LayoutResult, RenderNode, LayoutNode};
use crate::core::layout::LayoutError;

/// A utility to stack children vertically, handling page breaks and margin collapsing.
pub struct VerticalStacker;

impl VerticalStacker {
    /// Lays out children vertically into the provided context.
    /// Returns `Ok(None)` if all children fit.
    /// Returns `Ok(Some(remainder))` if a break occurred, where `remainder` is the list of children for the next page.
    pub fn layout_children(
        ctx: &mut LayoutContext,
        children: &mut Vec<RenderNode>,
    ) -> Result<Option<Vec<RenderNode>>, LayoutError> {
        let mut next_page_children = Vec::new();
        let mut split_occurred = false;

        // Iterate through children. We use an index to split the vector if needed.
        let mut split_index = None;
        let mut partial_node = None;

        for (i, child) in children.iter_mut().enumerate() {
            if split_occurred {
                // This shouldn't be reached if we break loop, but conceptually
                // subsequent children are pushed to next page.
                break;
            }

            match child.layout(ctx)? {
                LayoutResult::Full => continue,
                LayoutResult::Partial(remainder) => {
                    split_occurred = true;
                    partial_node = Some(remainder);
                    split_index = Some(i);
                    break;
                }
            }
        }

        if split_occurred {
            if let Some(idx) = split_index {
                // If we have a partial remainder of the split child, start with that.
                if let Some(p) = partial_node {
                    next_page_children.push(p);
                }
                // Move all subsequent children to the next page.
                // We drain starting from idx + 1 because the child at idx was split/processed.
                if idx + 1 < children.len() {
                    next_page_children.extend(children.drain((idx + 1)..));
                }
            }
            Ok(Some(next_page_children))
        } else {
            Ok(None)
        }
    }
}