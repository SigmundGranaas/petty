// src/core/layout/nodes/flex.rs

use crate::core::idf::IRNode;
use crate::core::layout::geom::{self, BoxConstraints};
use crate::core::layout::node::{
    FlexState, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::nodes::block::create_background_and_borders;
use crate::core::layout::nodes::taffy_utils::computed_style_to_taffy;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use taffy::prelude::*;

#[derive(Debug, Clone)]
struct FlexLayoutOutput {
    size: geom::Size,
    child_layouts: Vec<taffy::Layout>,
}

#[derive(Debug)]
pub struct FlexNode {
    id: Option<String>,
    // We generate a stable hash for caching if an ID isn't provided,
    // or we can use the pointer address in a real DOM.
    // For this implementation, we rely on the ID or skip caching.
    children: Vec<RenderNode>,
    style: Arc<ComputedStyle>,
}

impl FlexNode {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);

        let IRNode::FlexContainer {
            meta,
            children: ir_children,
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("FlexContainer", node.kind()));
        };

        let mut children = engine.build_layout_node_children(ir_children, style.clone())?;
        // Stable sort ensures items with same order stay in DOM order.
        children.sort_by_key(|c| c.style().flex.order);

        Ok(RenderNode::Flex(Box::new(FlexNode::new(
            meta.id.clone(),
            children,
            style,
        ))))
    }

    pub fn new(id: Option<String>, children: Vec<RenderNode>, style: Arc<ComputedStyle>) -> Self {
        Self {
            id,
            children,
            style,
        }
    }

    /// Generates a cache key. Ideally, this would use a stable Node ID.
    /// Here we hash the string ID if present.
    fn get_cache_key(&self) -> Option<u64> {
        self.id.as_ref().map(|id| {
            let mut s = DefaultHasher::new();
            id.hash(&mut s);
            s.finish()
        })
    }

    /// Computes the flex layout using a transient Taffy tree.
    fn compute_flex_layout_data(
        &self,
        env: &LayoutEnvironment,
        constraints: BoxConstraints,
    ) -> FlexLayoutOutput {
        let mut taffy = TaffyTree::<usize>::new();
        let mut child_nodes = Vec::with_capacity(self.children.len());

        for (i, child) in self.children.iter().enumerate() {
            let child_style = computed_style_to_taffy(child.style());
            // Safe to unwrap as Taffy operations in memory are reliable
            let node = taffy.new_leaf_with_context(child_style, i).unwrap();
            child_nodes.push(node);
        }

        let mut root_style = computed_style_to_taffy(&self.style);

        // Apply constraints
        if constraints.has_bounded_width() && self.style.box_model.width.is_none() {
            root_style.size.width = taffy::style::Dimension::length(constraints.max_width);
        }
        if constraints.is_tight() && self.style.box_model.height.is_none() {
            root_style.size.height = taffy::style::Dimension::length(constraints.max_height);
        }

        let root_node = taffy.new_with_children(root_style, &child_nodes).unwrap();

        let available_space = taffy::geometry::Size {
            width: if constraints.has_bounded_width() {
                taffy::style::AvailableSpace::Definite(constraints.max_width)
            } else {
                taffy::style::AvailableSpace::MaxContent
            },
            height: if constraints.has_bounded_height() {
                taffy::style::AvailableSpace::Definite(constraints.max_height)
            } else {
                taffy::style::AvailableSpace::MaxContent
            },
        };

        // Capture for closure
        let engine = env.engine;
        let page_index = env.local_page_index;

        taffy
            .compute_layout_with_measure(
                root_node,
                available_space,
                |known_dims, available_space, _node_id, context, _style| {
                    let Some(index) = context else {
                        return taffy::geometry::Size::ZERO;
                    };

                    let child = &self.children[*index];

                    let min_w = known_dims.width.unwrap_or(0.0);
                    let max_w = match available_space.width {
                        taffy::style::AvailableSpace::Definite(w) => w,
                        taffy::style::AvailableSpace::MaxContent => f32::INFINITY,
                        taffy::style::AvailableSpace::MinContent => 0.0,
                    };

                    // We don't constrain height during measure to allow content to grow
                    let child_constraints = BoxConstraints::new(min_w, max_w, 0.0, f32::INFINITY);

                    let size = child.measure(
                        &LayoutEnvironment {
                            engine,
                            local_page_index: page_index,
                        },
                        child_constraints,
                    );

                    taffy::geometry::Size {
                        width: size.width,
                        height: size.height,
                    }
                },
            )
            .unwrap();

        let root_layout = taffy.layout(root_node).unwrap();
        let size = geom::Size::new(root_layout.size.width, root_layout.size.height);

        let child_layouts = child_nodes
            .iter()
            .map(|&id| *taffy.layout(id).unwrap())
            .collect();

        FlexLayoutOutput {
            size,
            child_layouts,
        }
    }
}

impl LayoutNode for FlexNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> geom::Size {
        // We do not cache during measurement as it might happen with different constraints
        // often (speculative layout).
        self.compute_flex_layout_data(env, constraints).size
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            ctx.register_anchor(id);
        }

        let (start_index, mut child_resume_state) = if let Some(state) = break_state {
            let flex_state = state.as_flex()?;
            (
                flex_state.child_index,
                flex_state.child_state.map(|b| *b),
            )
        } else {
            (0, None)
        };

        let is_continuation = start_index > 0 || child_resume_state.is_some();

        if !is_continuation {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
            ctx.advance_cursor(margin_to_add);
        }
        ctx.last_v_margin = 0.0;

        let start_y = ctx.cursor_y();

        // --- Caching Logic Implementation ---
        let cache_key = self.get_cache_key();
        let layout_output = if let Some(key) = cache_key {
            if let Some(cached) = ctx.layout_cache.get(&key) {
                if let Some(output) = cached.downcast_ref::<FlexLayoutOutput>() {
                    output.clone()
                } else {
                    // Cache invalid or type mismatch, recompute
                    let env = LayoutEnvironment {
                        engine: ctx.engine,
                        local_page_index: ctx.local_page_index,
                    };
                    let output = self.compute_flex_layout_data(&env, constraints);
                    ctx.layout_cache.insert(key, Box::new(output.clone()));
                    output
                }
            } else {
                let env = LayoutEnvironment {
                    engine: ctx.engine,
                    local_page_index: ctx.local_page_index,
                };
                let output = self.compute_flex_layout_data(&env, constraints);
                ctx.layout_cache.insert(key, Box::new(output.clone()));
                output
            }
        } else {
            // No ID, no cache
            let env = LayoutEnvironment {
                engine: ctx.engine,
                local_page_index: ctx.local_page_index,
            };
            self.compute_flex_layout_data(&env, constraints)
        };
        // -------------------------------------

        let content_height = layout_output.size.height;

        let mut scroll_offset_y = 0.0;
        if start_index > 0 && start_index < layout_output.child_layouts.len() {
            scroll_offset_y = layout_output.child_layouts[start_index].location.y;
        }

        let bg_elements = create_background_and_borders(
            ctx.bounds(),
            &self.style,
            start_y,
            content_height,
            !is_continuation,
            true, // Optimistic: assumes it fits, will be clipped visually if split
        );
        for el in bg_elements {
            ctx.push_element(el);
        }

        let mut break_occurred = false;
        let mut next_state_index = 0;
        let mut next_child_state = None;

        const EPSILON: f32 = 0.01;
        const LAYOUT_SLACK: f32 = 0.5;
        let ctx_bounds = ctx.bounds();

        for (i, layout) in layout_output.child_layouts.iter().enumerate() {
            if i < start_index {
                continue;
            }

            let effective_y = layout.location.y - scroll_offset_y;
            let abs_y = start_y + effective_y;
            let child_h = layout.size.height;

            // Check if start of item is off-page
            if abs_y > ctx_bounds.height + EPSILON {
                break_occurred = true;
                next_state_index = i;
                break;
            }

            // Check if item ends off-page.
            if abs_y + child_h > ctx_bounds.height + EPSILON && i != start_index {
                break_occurred = true;
                next_state_index = i;
                break;
            }

            let available_h_on_page = (ctx_bounds.height - abs_y).max(0.0);

            let layout_bound_height = if child_h > available_h_on_page {
                available_h_on_page
            } else {
                child_h + LAYOUT_SLACK
            };

            let child_rect = geom::Rect {
                x: ctx_bounds.x + layout.location.x,
                y: ctx_bounds.y + abs_y,
                width: layout.size.width,
                height: layout_bound_height,
            };

            let child_constraints = BoxConstraints {
                min_width: layout.size.width,
                max_width: layout.size.width,
                min_height: 0.0,
                max_height: f32::INFINITY,
            };

            let child_resume = if i == start_index {
                child_resume_state.take()
            } else {
                None
            };

            let Ok(res) = ctx.with_child_bounds(child_rect, |child_ctx| {
                self.children[i].layout(child_ctx, child_constraints, child_resume)
            }) else {
                return Err(LayoutError::Generic(format!("Flex child {} layout failed", i)));
            };

            if let LayoutResult::Break(s) = res {
                break_occurred = true;
                next_state_index = i;
                next_child_state = Some(s);
                break;
            }
        }

        if break_occurred {
            ctx.set_cursor_y(ctx_bounds.height);
            Ok(LayoutResult::Break(NodeState::Flex(FlexState {
                child_index: next_state_index,
                child_state: next_child_state.map(Box::new),
            })))
        } else {
            let remaining_h = (content_height - scroll_offset_y).max(0.0);
            ctx.set_cursor_y(start_y + remaining_h + self.style.box_model.margin.bottom);
            Ok(LayoutResult::Finished)
        }
    }
}