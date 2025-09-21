mod drawing;
pub mod lopdf;
pub mod pdf;
pub mod renderer;
mod streaming_writer;

// Re-export the main renderer and the trait
pub use self::renderer::DocumentRenderer;