use crate::core::layout::{LayoutContext, LayoutError};
use std::any::Any;
use crate::core::layout::node::{LayoutResult, RenderNode};

pub struct VerticalStacker;

impl VerticalStacker {
    /// Lays out children vertically.
    /// Returns `Ok(LayoutResult::Finished)` if all children fit.
    /// Returns `Ok(LayoutResult::Break(state))` if a break occurred.
    pub fn layout_children(
        ctx: &mut LayoutContext,
        children: &[RenderNode],
        constraints: crate::core::layout::geom::BoxConstraints,
        start_index: usize,
        mut child_resume_state: Option<Box<dyn Any + Send>>,
        wrap_state: impl Fn(usize, Box<dyn Any + Send>) -> Box<dyn Any + Send>,
    ) -> Result<LayoutResult, LayoutError> {

        for (i, child) in children.iter().enumerate().skip(start_index) {
            let res = child.layout(ctx, constraints, child_resume_state.take())?;

            match res {
                LayoutResult::Finished => {
                    // Continue to next child
                }
                LayoutResult::Break(child_next_state) => {
                    // Wrap this state and return break
                    let state_to_return = Box::new(child_next_state); // Just wrap the child state directly? No, caller wraps it.
                    // Actually, the caller passed `wrap_state`.
                    // The caller typically wraps (index, child_state).
                    return Ok(LayoutResult::Break(wrap_state(i, state_to_return)));
                }
            }
        }

        Ok(LayoutResult::Finished)
    }
}