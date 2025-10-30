use crate::core::idf::InlineNode;
use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::style::ComputedStyle;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum LayoutAtom {
    Word {
        text: String,
        width: f32,
        style: Arc<ComputedStyle>,
        href: Option<String>,
    },
    Space {
        width: f32,
        style: Arc<ComputedStyle>,
    },
    Image {
        src: String,
        width: f32,
        height: f32,
        style: Arc<ComputedStyle>,
        href: Option<String>,
    },
    PageNumberPlaceholder {
        target_id: String,
        style: Arc<ComputedStyle>,
    },
    LineBreak,
}

impl LayoutAtom {
    pub fn width(&self) -> f32 {
        match self {
            LayoutAtom::Word { width, .. } => *width,
            LayoutAtom::Space { width, .. } => *width,
            LayoutAtom::Image { width, .. } => *width,
            LayoutAtom::PageNumberPlaceholder { style, .. } => {
                // Use a standard placeholder width for measurement.
                // The actual width will be determined by the renderer.
                style.font_size * 2.0 // Approx "XX"
            }
            LayoutAtom::LineBreak => 0.0,
        }
    }
    pub fn is_space(&self) -> bool {
        matches!(self, LayoutAtom::Space { .. })
    }
}

pub fn atomize_inlines(
    engine: &LayoutEngine,
    inlines: &[InlineNode],
    parent_style: &Arc<ComputedStyle>,
    parent_href: Option<String>,
) -> Vec<LayoutAtom> {
    let mut atoms = Vec::new();
    for inline in inlines {
        match inline {
            InlineNode::Text(text) => {
                for word in text.split_inclusive(' ') {
                    if word.ends_with(' ') {
                        let word_part = word.trim_end();
                        if !word_part.is_empty() {
                            atoms.push(LayoutAtom::Word {
                                text: word_part.to_string(),
                                width: engine.measure_text_width(word_part, parent_style),
                                style: parent_style.clone(),
                                href: parent_href.clone(),
                            });
                        }
                        atoms.push(LayoutAtom::Space {
                            width: engine.measure_text_width(" ", parent_style),
                            style: parent_style.clone(),
                        });
                    } else if !word.is_empty() {
                        atoms.push(LayoutAtom::Word {
                            text: word.to_string(),
                            width: engine.measure_text_width(word, parent_style),
                            style: parent_style.clone(),
                            href: parent_href.clone(),
                        });
                    }
                }
            }
            InlineNode::StyledSpan { meta, children } => {
                let style =
                    engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style);
                atoms.extend(atomize_inlines(engine, children, &style, parent_href.clone()));
            }
            InlineNode::Hyperlink {
                href,
                meta,
                children,
            } => {
                let style =
                    engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style);
                atoms.extend(atomize_inlines(
                    engine,
                    children,
                    &style,
                    Some(href.clone()),
                ));
            }
            InlineNode::Image { meta, src } => {
                let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style);
                let height = style.font_size; // Basic heuristic for inline image height
                let width = height; // Assume square for now
                atoms.push(LayoutAtom::Image {
                    src: src.clone(),
                    width,
                    height,
                    style,
                    href: parent_href.clone(),
                });
            }
            InlineNode::PageReference { target_id, meta, children } => {
                let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style);

                // FIX: Process children first, then add the placeholder.
                // This correctly handles text like `(see page <ref/>)`.
                atoms.extend(atomize_inlines(engine, children, &style, parent_href.clone()));
                atoms.push(LayoutAtom::PageNumberPlaceholder {
                    target_id: target_id.clone(),
                    style: style.clone(),
                });
            }
            InlineNode::LineBreak => {
                atoms.push(LayoutAtom::LineBreak);
            }
        }
    }
    atoms
}