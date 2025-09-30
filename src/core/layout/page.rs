use super::elements::{ImageElement, LayoutElement, PositionedElement, RectElement};
use super::style;
use super::{LayoutBox, LayoutContent, LayoutEngine, Rect};
use crate::core::style::dimension::Margins;
use std::collections::VecDeque;

struct TraversalState {
    iterator: std::vec::IntoIter<LayoutBox>,
    offset: Rect,
}

/// A stateful iterator that performs the **Pagination Pass** of the layout algorithm.
/// It consumes a pre-computed `LayoutBox` tree and yields pages of `PositionedElement`s.
pub struct PageIterator<'a> {
    engine: &'a LayoutEngine,
    // A stack of iterators over children, allowing us to traverse the tree without recursion.
    traversal_stack: Vec<TraversalState>,
    // A queue for boxes that are ready to be placed on a page.
    pending_boxes: VecDeque<LayoutBox>,
    _page_height: f32,
    content_bottom_y: f32,
    margins: &'a Margins,
    is_finished: bool,
    // An offset to adjust Y positions for pagination. For page 1, it's 0.
    // For page 2+, it's a negative value to shift content up.
    page_y_offset: f32,
}

impl<'a> PageIterator<'a> {
    pub fn new(root_box: LayoutBox, engine: &'a LayoutEngine) -> Self {
        let (_page_width, page_height) = style::get_page_dimensions(&engine.stylesheet);
        let margins = &engine.stylesheet.page.margins;
        let content_bottom_y = page_height - margins.bottom - engine.stylesheet.page.footer_height;

        let root_children = if let LayoutContent::Children(c) = root_box.content {
            c
        } else {
            vec![]
        };

        Self {
            engine,
            traversal_stack: vec![TraversalState {
                iterator: root_children.into_iter(),
                offset: Rect {
                    x: margins.left,
                    y: margins.top,
                    ..Default::default()
                },
            }],
            pending_boxes: VecDeque::new(),
            _page_height: page_height,
            content_bottom_y,
            margins,
            is_finished: false,
            page_y_offset: 0.0,
        }
    }
}

impl<'a> Iterator for PageIterator<'a> {
    type Item = Vec<PositionedElement>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_finished {
            return None;
        }

        let mut page_elements = Vec::new();
        let mut work_was_done = false;

        'page_loop: loop {
            let layout_box = if let Some(pending) = self.pending_boxes.pop_front() {
                work_was_done = true;
                pending
            } else {
                let Some(next_box) = self.flatten_next_node() else {
                    self.is_finished = true;
                    break 'page_loop;
                };
                work_was_done = true;
                next_box
            };

            let absolute_y_infinite = layout_box.rect.y;
            let absolute_y_on_page = absolute_y_infinite + self.page_y_offset;

            // Page break check: Does the element's bottom edge go past the content area?
            // Use a small epsilon to handle floating point inaccuracies.
            if absolute_y_on_page + layout_box.rect.height > self.content_bottom_y + 0.001 {
                let fresh_page_height = self.content_bottom_y - self.margins.top;
                if layout_box.rect.height > fresh_page_height {
                    log::error!(
                        "Element with height {:.2} exceeds page content height {:.2}. Skipping.",
                        layout_box.rect.height, fresh_page_height
                    );
                    if matches!(layout_box.content, LayoutContent::Color) {
                        self.traversal_stack.pop();
                    }
                    continue 'page_loop;
                }

                let new_page_top_y = self.margins.top;
                self.page_y_offset = new_page_top_y - absolute_y_infinite;

                self.pending_boxes.push_front(layout_box);

                if !page_elements.is_empty() {
                    return Some(page_elements);
                } else {
                    continue 'page_loop;
                }
            }

            let positioned_el = PositionedElement {
                x: layout_box.rect.x,
                y: absolute_y_on_page,
                width: layout_box.rect.width,
                height: layout_box.rect.height,
                element: match layout_box.content {
                    LayoutContent::Text(content, href) => {
                        LayoutElement::Text(super::TextElement { content, href })
                    }
                    LayoutContent::Image(src) => LayoutElement::Image(ImageElement { src }),
                    LayoutContent::Color => LayoutElement::Rectangle(RectElement),
                    _ => unreachable!("Flattened node should not be a container."),
                },
                style: layout_box.style.clone(),
            };
            page_elements.push(positioned_el);
        }

        if work_was_done || !page_elements.is_empty() {
            Some(page_elements)
        } else {
            None
        }
    }
}

impl<'a> PageIterator<'a> {
    fn flatten_next_node(&mut self) -> Option<LayoutBox> {
        loop {
            let Some(current_level) = self.traversal_stack.last_mut() else {
                return None;
            };

            let Some(mut next_box) = current_level.iterator.next() else {
                self.traversal_stack.pop();
                continue;
            };

            next_box.rect.x += current_level.offset.x;
            next_box.rect.y += current_level.offset.y;

            let has_visible_style = next_box.style.background_color.is_some();

            match next_box.content {
                LayoutContent::Children(children) => {
                    let new_offset = next_box.rect;
                    self.traversal_stack.push(TraversalState {
                        iterator: children.into_iter(),
                        offset: new_offset,
                    });
                    if has_visible_style {
                        return Some(LayoutBox {
                            content: LayoutContent::Color,
                            ..next_box
                        });
                    }
                }
                _ => {
                    return Some(next_box);
                }
            }
        }
    }
}