//! Contains the implementations of the `LayoutNode` trait for each document element type.

pub mod block;
pub mod flex;
pub mod heading;
pub mod image;
pub mod index_marker;
pub mod list;
pub mod list_item;
pub mod page_break;
pub mod paragraph;
pub mod table;

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