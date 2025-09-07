// This module is responsible for processing the event stream from the parser,
// calculating element positions, sizes, and handling page breaks.

mod model;
mod processor;

pub(crate) use model::*;
pub(crate) use processor::StreamingLayoutProcessor;