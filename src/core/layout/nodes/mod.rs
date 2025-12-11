//! Contains the implementations of the LayoutNode trait for each document element type.

pub mod block;
pub mod flex;
pub mod heading;
pub mod image;
pub mod index_marker;
pub mod list;
pub mod list_item;
pub mod list_utils;
pub mod page_break;
pub mod paragraph;
pub mod paragraph_utils;
pub mod table;
pub mod table_solver; // Register the new solver module
pub mod taffy_utils;

#[cfg(test)]
mod block_test;
#[cfg(test)]
mod block_split_test;
#[cfg(test)]
mod flex_test;
#[cfg(test)]
mod image_test;
#[cfg(test)]
mod index_marker_test;
#[cfg(test)]
mod table_test;
#[cfg(test)]
mod list_test;

use crate::core::idf::TextStr;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult, NodeState};
use crate::core::layout::{LayoutEnvironment, LayoutError};
use crate::core::layout::style::ComputedStyle;

// Import specific nodes for the macro
use self::block::BlockNode;
use self::flex::FlexNode;
use self::heading::HeadingNode;
use self::image::ImageNode;
use self::index_marker::IndexMarkerNode;
use self::list::ListNode;
use self::list_item::ListItemNode;
use self::page_break::PageBreakNode;
use self::paragraph::ParagraphNode;
use self::table::TableNode;

macro_rules! define_render_node {
    ( $( $variant:ident ( $node_struct:ident ) ),* ) => {
        #[derive(Debug, Clone, Copy)]
        pub enum RenderNode<'a> {
            $( $variant(&'a $node_struct<'a>), )*
        }

        impl<'a> RenderNode<'a> {
            pub fn kind_str(&self) -> &'static str {
                match self {
                    $( Self::$variant(_) => stringify!($variant), )*
                }
            }
        }

        impl<'a> LayoutNode for RenderNode<'a> {
            fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Result<Size, LayoutError> {
                match self {
                    $( Self::$variant(n) => n.measure(env, constraints), )*
                }
            }

            fn layout(
                &self,
                ctx: &mut LayoutContext,
                constraints: BoxConstraints,
                break_state: Option<NodeState>,
            ) -> Result<LayoutResult, LayoutError> {
                match self {
                    $( Self::$variant(n) => n.layout(ctx, constraints, break_state), )*
                }
            }

            fn style(&self) -> &ComputedStyle {
                match self {
                    $( Self::$variant(n) => n.style(), )*
                }
            }

            fn check_for_page_break(&self) -> Option<Option<TextStr>> {
                 match self {
                    $( Self::$variant(n) => n.check_for_page_break(), )*
                }
            }
        }
    };
}

define_render_node!(
    Block(BlockNode),
    Flex(FlexNode),
    Heading(HeadingNode),
    Image(ImageNode),
    IndexMarker(IndexMarkerNode),
    List(ListNode),
    ListItem(ListItemNode),
    PageBreak(PageBreakNode),
    Paragraph(ParagraphNode),
    Table(TableNode)
);