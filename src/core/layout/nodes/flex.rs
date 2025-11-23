use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::nodes::block::draw_background_and_borders;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use std::sync::Arc;
use crate::core::idf::IRNode;

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

        Ok(RenderNode::Flex(FlexNode::new_from_children(id, children, style)))
    }
}

#[derive(Debug, Clone)]
pub struct FlexNode {
    id: Option<String>,
    children: Vec<RenderNode>,
    style: Arc<ComputedStyle>,
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
        }
    }

    fn compute_layout(
        &mut self,
        env: &LayoutEnvironment,
        constraints: BoxConstraints,
    ) -> FlexLayoutOutput {
        // 1. Resolve container dimensions and box model
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let v_deduction = self.style.padding_y() + self.style.border_y();

        // Determine available space for content
        let style_width_pt = if let Some(Dimension::Pt(w)) = self.style.box_model.width { Some(w) } else { None };
        let style_height_pt = if let Some(Dimension::Pt(h)) = self.style.box_model.height { Some(h) } else { None };

        let max_content_width = if let Some(w) = style_width_pt {
            (w - h_deduction).max(0.0)
        } else if constraints.has_bounded_width() {
            (constraints.max_width - h_deduction).max(0.0)
        } else {
            f32::INFINITY
        };

        let max_content_height = if let Some(h) = style_height_pt {
            (h - v_deduction).max(0.0)
        } else if constraints.has_bounded_height() {
            (constraints.max_height - v_deduction).max(0.0)
        } else {
            f32::INFINITY
        };

        // Determine main and cross axes helpers
        let is_row = matches!(
            self.style.flex.direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let is_reverse_main = matches!(
            self.style.flex.direction,
            FlexDirection::RowReverse | FlexDirection::ColumnReverse
        );

        let helper = AxisHelper::new(is_row);

        let available_main = if is_row { max_content_width } else { max_content_height };
        let available_cross = if is_row { max_content_height } else { max_content_width };

        // 2. Construct Flex Items
        let items: Vec<FlexItem> = self.children.iter_mut().enumerate().map(|(i, child)| {
            let child_style = child.style();
            let flex_grow = child_style.flex.grow;
            let flex_shrink = child_style.flex.shrink;

            let basis_dim = &child_style.flex.basis;
            let width_dim = &child_style.box_model.width;
            let height_dim = &child_style.box_model.height;

            let basis = match basis_dim {
                Dimension::Pt(v) => *v,
                Dimension::Percent(p) => if available_main.is_finite() { available_main * p / 100.0 } else { 0.0 },
                Dimension::Auto => {
                    let main_dim = if is_row { width_dim } else { height_dim };
                    match main_dim {
                        Some(Dimension::Pt(v)) => *v,
                        Some(Dimension::Percent(p)) => if available_main.is_finite() { available_main * p / 100.0 } else { 0.0 },
                        _ => {
                            let measure_constraints = BoxConstraints {
                                min_width: 0.0, max_width: f32::INFINITY,
                                min_height: 0.0, max_height: f32::INFINITY
                            };
                            let size = child.measure(env, measure_constraints);
                            helper.main(size)
                        }
                    }
                }
            };

            FlexItem {
                index: i,
                base_size: basis,
                flex_grow,
                flex_shrink,
                target_main: basis,
                target_cross: 0.0,
                offset_main: 0.0,
                offset_cross: 0.0,
            }
        }).collect();

        // 3. Line Breaking
        let mut lines: Vec<FlexLine> = Vec::new();
        let wrap = self.style.flex.wrap != FlexWrap::NoWrap;

        let mut current_line_items = Vec::new();
        let mut current_main_size = 0.0;

        for item in items {
            let margins = self.children[item.index].style().box_model.margin.clone();
            let item_outer_main = item.base_size + helper.main_margin(&margins);

            if wrap && !current_line_items.is_empty() && available_main.is_finite() && (current_main_size + item_outer_main > available_main) {
                lines.push(FlexLine {
                    items: current_line_items,
                    main_size: current_main_size,
                    cross_size: 0.0,
                });
                current_line_items = Vec::new();
                current_main_size = 0.0;
            }

            current_main_size += item_outer_main;
            current_line_items.push(item);
        }
        if !current_line_items.is_empty() {
            lines.push(FlexLine {
                items: current_line_items,
                main_size: current_main_size,
                cross_size: 0.0,
            });
        }

        // 4. Resolve Flexible Lengths
        for line in &mut lines {
            if available_main.is_finite() {
                let total_outer: f32 = line.items.iter().map(|item| {
                    let m = self.children[item.index].style().box_model.margin.clone();
                    item.base_size + helper.main_margin(&m)
                }).sum();

                let free_space = available_main - total_outer;

                if free_space > 0.0 {
                    let total_grow: f32 = line.items.iter().map(|i| i.flex_grow).sum();
                    if total_grow > 0.0 {
                        for item in &mut line.items {
                            let share = item.flex_grow / total_grow;
                            item.target_main = item.base_size + free_space * share;
                        }
                    }
                } else if free_space < 0.0 {
                    let total_shrink_scaled: f32 = line.items.iter().map(|i| i.flex_shrink * i.base_size).sum();
                    if total_shrink_scaled > 0.0 {
                        for item in &mut line.items {
                            let ratio = (item.flex_shrink * item.base_size) / total_shrink_scaled;
                            item.target_main = item.base_size + free_space * ratio;
                        }
                    }
                }
            }
        }

        // 5. Cross Size Determination
        for line in &mut lines {
            let mut max_cross: f32 = 0.0;
            for item in &mut line.items {
                let child_constraints = if is_row {
                    BoxConstraints {
                        min_width: item.target_main, max_width: item.target_main,
                        min_height: 0.0, max_height: f32::INFINITY
                    }
                } else {
                    BoxConstraints {
                        min_width: 0.0, max_width: f32::INFINITY,
                        min_height: item.target_main, max_height: item.target_main
                    }
                };

                let size = self.children[item.index].measure(env, child_constraints);
                item.target_cross = helper.cross(size);

                let margins = self.children[item.index].style().box_model.margin.clone();
                let outer_cross = item.target_cross + helper.cross_margin(&margins);
                max_cross = max_cross.max(outer_cross);
            }
            line.cross_size = max_cross;
        }

        // Handle 'align-items: stretch'
        let align_items = self.style.flex.align_items.clone();
        for line in &mut lines {
            for item in &mut line.items {
                let child_style = self.children[item.index].style();
                let align_self = if child_style.flex.align_self == AlignSelf::Auto {
                    match align_items {
                        AlignItems::Stretch => AlignSelf::Stretch,
                        AlignItems::Center => AlignSelf::Center,
                        AlignItems::FlexStart => AlignSelf::FlexStart,
                        AlignItems::FlexEnd => AlignSelf::FlexEnd,
                        AlignItems::Baseline => AlignSelf::FlexStart,
                    }
                } else {
                    child_style.flex.align_self.clone()
                };

                if align_self == AlignSelf::Stretch {
                    let margins = child_style.box_model.margin.clone();
                    let margin_cross = helper.cross_margin(&margins);
                    let new_cross = (line.cross_size - margin_cross).max(0.0);

                    let (w, h) = if is_row {
                        (item.target_main, new_cross)
                    } else {
                        (new_cross, item.target_main)
                    };
                    let constraints = BoxConstraints::tight(Size::new(w, h));
                    self.children[item.index].measure(env, constraints);
                    item.target_cross = new_cross;
                }
            }
        }

        // 6. Main Axis Alignment
        for line in &mut lines {
            let total_main: f32 = line.items.iter().map(|i| {
                let m = self.children[i.index].style().box_model.margin.clone();
                i.target_main + helper.main_margin(&m)
            }).sum();

            let free = (available_main - total_main).max(0.0);
            let num_items = line.items.len();

            let (start_offset, gap) = match self.style.flex.justify_content {
                JustifyContent::FlexStart => (0.0, 0.0),
                JustifyContent::FlexEnd => (free, 0.0),
                JustifyContent::Center => (free / 2.0, 0.0),
                JustifyContent::SpaceBetween => if num_items > 1 { (0.0, free / (num_items - 1) as f32) } else { (0.0, 0.0) },
                JustifyContent::SpaceAround => if num_items > 0 { (free / num_items as f32 / 2.0, free / num_items as f32) } else { (0.0, 0.0) },
                JustifyContent::SpaceEvenly => if num_items > 0 { (free / (num_items + 1) as f32, free / (num_items + 1) as f32) } else { (0.0, 0.0) },
            };

            let mut cursor = start_offset;
            for item in &mut line.items {
                let m = self.children[item.index].style().box_model.margin.clone();
                let margin_start = if is_row { m.left } else { m.right };
                let margin_end = if is_row { m.right } else { m.left };

                item.offset_main = cursor + margin_start;
                cursor += margin_start + item.target_main + margin_end + gap;
            }
        }

        // 7. Cross Axis Alignment
        let total_lines_cross: f32 = lines.iter().map(|l| l.cross_size).sum();

        if available_cross.is_finite() && total_lines_cross < available_cross {
            let free_cross = available_cross - total_lines_cross;
            match self.style.flex.align_content {
                JustifyContent::FlexStart => { if lines.len() == 1 { lines[0].cross_size = available_cross; } },
                JustifyContent::FlexEnd => { if lines.len() == 1 { lines[0].cross_size = available_cross; } },
                _ => {
                    if !lines.is_empty() {
                        let extra = free_cross / lines.len() as f32;
                        for line in &mut lines { line.cross_size += extra; }
                    }
                }
            }
        }

        let mut line_cross_cursor = 0.0;
        for line in &mut lines {
            for item in &mut line.items {
                let child_style = self.children[item.index].style();
                let m = child_style.box_model.margin.clone();
                let margin_cross_start = if is_row { m.top } else { m.left };

                let free_cross = line.cross_size - (item.target_cross + helper.cross_margin(&m));

                let align_self = if child_style.flex.align_self == AlignSelf::Auto {
                    match self.style.flex.align_items {
                        AlignItems::FlexEnd => AlignSelf::FlexEnd,
                        AlignItems::Center => AlignSelf::Center,
                        _ => AlignSelf::FlexStart,
                    }
                } else {
                    child_style.flex.align_self.clone()
                };

                let alignment_offset = match align_self {
                    AlignSelf::FlexEnd => free_cross,
                    AlignSelf::Center => free_cross / 2.0,
                    _ => 0.0,
                };
                item.offset_cross = line_cross_cursor + alignment_offset + margin_cross_start;
            }
            line_cross_cursor += line.cross_size;
        }

        if is_reverse_main && available_main.is_finite() {
            for line in &mut lines {
                for item in &mut line.items {
                    item.offset_main = available_main - item.offset_main - item.target_main;
                }
            }
        }

        let computed_main = if available_main.is_finite() { available_main } else {
            lines.iter().map(|l| l.main_size).fold(0.0, f32::max)
        };
        let computed_cross = if available_cross.is_finite() { available_cross } else {
            lines.iter().map(|l| l.cross_size).sum()
        };

        let (final_w, final_h) = if is_row {
            (computed_main + h_deduction, computed_cross + v_deduction)
        } else {
            (computed_cross + h_deduction, computed_main + v_deduction)
        };

        FlexLayoutOutput {
            size: Size::new(final_w, final_h),
            lines,
            content_offset: (self.style.border_left_width() + self.style.box_model.padding.left, self.style.border_top_width() + self.style.box_model.padding.top),
            is_row,
        }
    }
}

struct FlexLayoutOutput {
    size: Size,
    lines: Vec<FlexLine>,
    content_offset: (f32, f32),
    is_row: bool,
}

struct FlexLine {
    items: Vec<FlexItem>,
    main_size: f32,
    cross_size: f32,
}

#[derive(Clone, Debug)]
struct FlexItem {
    index: usize,
    base_size: f32,
    flex_grow: f32,
    flex_shrink: f32,
    target_main: f32,
    target_cross: f32,
    offset_main: f32,
    offset_cross: f32,
}

impl LayoutNode for FlexNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let output = self.compute_layout(env, constraints);
        output.size
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutContext,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }

        let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
        ctx.advance_cursor(margin_to_add);
        ctx.last_v_margin = 0.0;

        let start_y = ctx.cursor.1;

        let constraints = BoxConstraints::tight_width(ctx.bounds.width);
        let layout_output = self.compute_layout(&LayoutEnvironment { engine: ctx.engine, local_page_index: ctx.local_page_index }, constraints);

        let content_height = layout_output.size.height
            - self.style.padding_y()
            - self.style.border_y();

        draw_background_and_borders(
            ctx.elements,
            ctx.bounds,
            &self.style,
            start_y,
            content_height
        );

        let (offset_x, offset_y) = layout_output.content_offset;
        let abs_content_start_y = start_y + offset_y;

        let mut next_page_children = Vec::new();
        let mut break_occurred = false;

        for line in layout_output.lines {
            if break_occurred {
                for item in line.items {
                    next_page_children.push(self.children[item.index].clone());
                }
                continue;
            }

            let mut line_min_y = f32::INFINITY;
            let mut line_max_y = f32::NEG_INFINITY;
            for item in &line.items {
                let y = if layout_output.is_row { item.offset_cross } else { item.offset_main };
                let h = if layout_output.is_row { item.target_cross } else { item.target_main };
                line_min_y = line_min_y.min(y);
                line_max_y = line_max_y.max(y + h);
            }
            if line.items.is_empty() { continue; }

            let abs_line_bottom = abs_content_start_y + line_max_y;
            let fits_fully = abs_line_bottom <= ctx.bounds.height + 0.1;
            let is_at_top = start_y < 1.0;

            if !fits_fully && !is_at_top {
                break_occurred = true;
                for item in line.items {
                    next_page_children.push(self.children[item.index].clone());
                }
                continue;
            }

            for item in line.items {
                if break_occurred {
                    next_page_children.push(self.children[item.index].clone());
                    continue;
                }

                let (x, y) = if layout_output.is_row {
                    (item.offset_main, item.offset_cross)
                } else {
                    (item.offset_cross, item.offset_main)
                };

                let absolute_item_y = abs_content_start_y + y;
                let item_height = if layout_output.is_row { item.target_cross } else { item.target_main };
                let target_w = if layout_output.is_row { item.target_main } else { item.target_cross };

                let space_on_page = (ctx.bounds.height - absolute_item_y).max(0.0);
                let effective_h = item_height.min(space_on_page);

                let child_rect = geom::Rect {
                    x: ctx.bounds.x + offset_x + x,
                    y: ctx.bounds.y + absolute_item_y,
                    width: target_w,
                    height: effective_h,
                };

                let child_constraints = BoxConstraints::tight(Size::new(target_w, item_height));
                let measure_env = LayoutEnvironment { engine: ctx.engine, local_page_index: ctx.local_page_index };
                self.children[item.index].measure(&measure_env, child_constraints);

                // Use the child context helper
                ctx.with_child_bounds(child_rect, |child_ctx| {
                    match self.children[item.index].layout(child_ctx) {
                        Ok(LayoutResult::Full) => {}
                        Ok(LayoutResult::Partial(remainder)) => {
                            break_occurred = true;
                            next_page_children.push(remainder);
                        }
                        Err(e) => log::warn!("Flex item layout error: {}", e),
                    }
                });
            }
        }

        ctx.cursor.1 = start_y + layout_output.size.height + self.style.box_model.margin.bottom;

        if break_occurred {
            if !next_page_children.is_empty() {
                let mut next_style = (*self.style).clone();
                next_style.box_model.margin.top = 0.0;
                next_style.border.top = None;
                next_style.box_model.padding.top = 0.0;
                if next_style.box_model.height.is_some() {
                    next_style.box_model.height = None;
                }

                let remainder = FlexNode::new_from_children(self.id.clone(), next_page_children, Arc::new(next_style));
                return Ok(LayoutResult::Partial(RenderNode::Flex(remainder)));
            }
        }

        Ok(LayoutResult::Full)
    }
}

struct AxisHelper {
    is_row: bool,
}

impl AxisHelper {
    fn new(is_row: bool) -> Self {
        Self { is_row }
    }
    fn main(&self, size: Size) -> f32 {
        if self.is_row { size.width } else { size.height }
    }
    fn cross(&self, size: Size) -> f32 {
        if self.is_row { size.height } else { size.width }
    }
    fn main_margin(&self, m: &Margins) -> f32 {
        if self.is_row { m.left + m.right } else { m.top + m.bottom }
    }
    fn cross_margin(&self, m: &Margins) -> f32 {
        if self.is_row { m.top + m.bottom } else { m.left + m.right }
    }
}