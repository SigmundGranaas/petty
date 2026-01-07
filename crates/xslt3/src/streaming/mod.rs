pub mod accumulator;
pub mod analysis;
pub mod context;
pub mod event_model;
pub mod executor;

pub use accumulator::{
    AccumulatorDefinition, AccumulatorPhase, AccumulatorRule, AccumulatorRuntime,
};
pub use analysis::{Posture, StreamabilityAnalyzer, StreamabilityResult, Sweep, Usage};
pub use context::{StreamedContext, StreamedNode, StreamedNodeKind};
pub use event_model::{AncestorInfo, Attribute, QName, StreamEvent, StreamEventHandler};
pub use executor::{
    StreamingExecutor, StreamingResult, parse_and_stream, parse_and_stream_with_accumulators,
};
