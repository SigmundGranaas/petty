use crate::core::layout::nodes::block::create_background_and_borders;
use crate::core::layout::nodes::taffy_utils::computed_style_to_taffy;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, AnchorLocation, LayoutContext, LayoutEngine, LayoutEnvironment, LayoutError};
use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use std::sync::{Arc, Mutex};
use std::any::Any;
use taffy::prelude::*;
use crate::core::layout::geom::BoxConstraints;
use crate::core::layout::node::{LayoutNode, LayoutResult, RenderNode};

pub struct FlexBuilder;

impl NodeBuilder for FlexBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (id, ir_children) = match node {
            IRNode::FlexContainer { meta, children } => (meta.id.clone(), children),
            _ => return Err(LayoutError::BuilderMismatch("FlexContainer", node.kind())),
        };
        let mut children = engine.build_layout_node_children(ir_children, style.clone())?;

        // Stable sort ensures items with same order stay in DOM order.
        children.sort_by_key(|c| c.style().flex.order);

        Ok(Box::new(FlexNode::new_from_children(id, children, style)))
    }
}

const CACHE_SIZE: usize = 4;

#[derive(Debug, Clone)]
struct FlexLayoutOutput {
    size: geom::Size,
    child_layouts: Vec<taffy::Layout>,
}

#[derive(Debug)]
struct TaffyCache {
    tree: TaffyTree<usize>,
    root: NodeId,
    children: Vec<NodeId>,
}

// Safety: TaffyTree is logically Send, but contains raw pointers for optimization (CompactLength).
// We assert that we are not relying on thread-local storage or pointer aliasing across threads
// that would make moving this tree to another thread unsafe.
unsafe impl Send for TaffyCache {}

#[derive(Debug)]
pub struct FlexNode {
    id: Option<String>,
    children: Vec<RenderNode>,
    style: Arc<ComputedStyle>,
    layout_cache: Mutex<Vec<(BoxConstraints, FlexLayoutOutput)>>,
    /// Persistent Taffy instance to avoid re-allocating the tree on every layout/measure.
    taffy_instance: Mutex<Option<TaffyCache>>,
}

#[derive(Debug)]
struct FlexState {
    child_index: usize,
    child_state: Option<Box<dyn Any + Send>>,
}

impl FlexNode {
    pub fn new_from_children(
        id: Option<String>,
        children: Vec<RenderNode>,
        style: Arc<ComputedStyle>,
    ) -> Self {
        Self {
            id,
            children,
            style,
            layout_cache: Mutex::new(Vec::with_capacity(CACHE_SIZE)),
            taffy_instance: Mutex::new(None),
        }
    }

    fn compute_flex_layout_data(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> FlexLayoutOutput {
        if let Ok(cache) = self.layout_cache.lock() {
            for (c, output) in cache.iter() {
                if *c == constraints {
                    return output.clone();
                }
            }
        }

        let mut taffy_guard = self.taffy_instance.lock().unwrap();

        if taffy_guard.is_none() {
            let mut taffy = TaffyTree::<usize>::new();
            let mut child_nodes = Vec::with_capacity(self.children.len());

            for (i, child) in self.children.iter().enumerate() {
                let child_style = computed_style_to_taffy(child.style());
                let node = taffy.new_leaf_with_context(child_style, i).unwrap();
                child_nodes.push(node);
            }

            let root_style = computed_style_to_taffy(&self.style);
            let root_node = taffy.new_with_children(root_style, &child_nodes).unwrap();

            *taffy_guard = Some(TaffyCache {
                tree: taffy,
                root: root_node,
                children: child_nodes,
            });
        }

        let cache_entry = taffy_guard.as_mut().unwrap();
        let taffy = &mut cache_entry.tree;
        let root_node = cache_entry.root;
        let child_nodes = &cache_entry.children;

        // Update root style size based on constraints
        let mut root_style = computed_style_to_taffy(&self.style);
        if constraints.has_bounded_width() && self.style.box_model.width.is_none() {
            root_style.size.width = taffy::style::Dimension::length(constraints.max_width);
        }
        if constraints.is_tight() && self.style.box_model.height.is_none() {
            root_style.size.height = taffy::style::Dimension::length(constraints.max_height);
        }
        let _ = taffy.set_style(root_node, root_style);

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

        let engine = env.engine;
        let page_index = env.local_page_index;

        taffy.compute_layout_with_measure(root_node, available_space, |known_dims, available_space, _node_id, context, _style| {
            if let Some(index) = context {
                let child = &self.children[*index];

                let min_w = known_dims.width.unwrap_or(0.0);
                let max_w = match available_space.width {
                    taffy::style::AvailableSpace::Definite(w) => w,
                    taffy::style::AvailableSpace::MaxContent => f32::INFINITY,
                    taffy::style::AvailableSpace::MinContent => 0.0,
                };

                let min_h = known_dims.height.unwrap_or(0.0);
                let max_h = match available_space.height {
                    taffy::style::AvailableSpace::Definite(h) => h,
                    _ => f32::INFINITY,
                };

                let constraints = BoxConstraints::new(min_w, max_w, min_h, max_h);
                let size = child.measure(&LayoutEnvironment { engine, local_page_index: page_index }, constraints);

                taffy::geometry::Size { width: size.width, height: size.height }
            } else {
                taffy::geometry::Size::ZERO
            }
        }).unwrap();

        let root_layout = taffy.layout(root_node).unwrap();
        let size = geom::Size::new(root_layout.size.width, root_layout.size.height);

        let mut child_layouts = Vec::with_capacity(self.children.len());
        for &child_node_id in child_nodes.iter() {
            child_layouts.push(*taffy.layout(child_node_id).unwrap());
        }

        let output = FlexLayoutOutput { size, child_layouts };

        if let Ok(mut cache) = self.layout_cache.lock() {
            if cache.len() >= CACHE_SIZE {
                cache.remove(0);
            }
            cache.push((constraints, output.clone()));
        }

        output
    }
}

impl LayoutNode for FlexNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> geom::Size {
        let output = self.compute_flex_layout_data(env, constraints);
        output.size
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }

        let (start_index, mut child_break_state) = if let Some(state) = break_state {
            let s = *state.downcast::<FlexState>().map_err(|_| LayoutError::Generic("Invalid FlexState".into()))?;
            (s.child_index, s.child_state)
        } else {
            (0, None)
        };

        let is_continuation = start_index > 0 || child_break_state.is_some();

        if !is_continuation {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
            ctx.advance_cursor(margin_to_add);
        }
        ctx.last_v_margin = 0.0;

        let start_y = ctx.cursor.1;

        let env = LayoutEnvironment { engine: ctx.engine, local_page_index: ctx.local_page_index };
        let layout_output = self.compute_flex_layout_data(&env, constraints);
        let content_height = layout_output.size.height;

        let mut scroll_offset_y = 0.0;
        if start_index > 0 && start_index < layout_output.child_layouts.len() {
            scroll_offset_y = layout_output.child_layouts[start_index].location.y;
        }

        let bg_elements = create_background_and_borders(
            ctx.bounds,
            &self.style,
            start_y,
            content_height,
            !is_continuation,
            true
        );
        ctx.elements.extend(bg_elements);

        let mut break_occurred = false;
        let mut next_state_index = 0;
        let mut next_child_state = None;

        const EPSILON: f32 = 0.01;
        const LAYOUT_SLACK: f32 = 0.5;

        for (i, layout) in layout_output.child_layouts.iter().enumerate() {
            if i < start_index { continue; }

            let child_x = layout.location.x;
            let child_y = layout.location.y;
            let child_w = layout.size.width;
            let child_h = layout.size.height;

            let effective_y = child_y - scroll_offset_y;
            let abs_y = start_y + effective_y;

            if abs_y > ctx.bounds.height + EPSILON {
                break_occurred = true;
                next_state_index = i;
                break;
            }

            if abs_y + child_h > ctx.bounds.height + EPSILON {
                if i != start_index {
                    break_occurred = true;
                    next_state_index = i;
                    break;
                }
            }

            let available_h_on_page = (ctx.bounds.height - abs_y).max(0.0);

            let layout_bound_height = if child_h > available_h_on_page {
                available_h_on_page
            } else {
                child_h + LAYOUT_SLACK
            };

            let child_rect = geom::Rect {
                x: ctx.bounds.x + child_x,
                y: ctx.bounds.y + abs_y,
                width: child_w,
                height: layout_bound_height,
            };

            let child_constraints = BoxConstraints {
                min_width: child_w,
                max_width: child_w,
                min_height: 0.0,
                max_height: f32::INFINITY,
            };

            let child_resume = if i == start_index { child_break_state.take() } else { None };

            let res = ctx.with_child_bounds(child_rect, |child_ctx| {
                self.children[i].layout(child_ctx, child_constraints, child_resume)
            })?;

            if let LayoutResult::Break(s) = res {
                break_occurred = true;
                next_state_index = i;
                next_child_state = Some(s);
                break;
            }
        }

        if break_occurred {
            ctx.cursor.1 = ctx.bounds.height;
            Ok(LayoutResult::Break(Box::new(FlexState {
                child_index: next_state_index,
                child_state: next_child_state,
            })))
        } else {
            let remaining_h = (content_height - scroll_offset_y).max(0.0);
            ctx.cursor.1 = start_y + remaining_h + self.style.box_model.margin.bottom;
            Ok(LayoutResult::Finished)
        }
    }
}