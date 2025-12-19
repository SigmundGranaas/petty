use crate::interface::{BlockState, NodeState};

/// Wraps a generic child state into a BlockState for block-like containers.
pub fn wrap_in_block_state(
    child_index: usize,
    child_state: NodeState,
) -> NodeState {
    NodeState::Block(BlockState {
        child_index,
        child_state: Some(Box::new(child_state)),
    })
}

/// Robust floating point comparison for layout calculations.
/// Handles `Option<f32>` to support `None` representing unbounded/infinite constraints.
pub fn floats_fuzzy_eq(a: Option<f32>, b: Option<f32>) -> bool {
    const EPSILON: f32 = 0.01;
    match (a, b) {
        (Some(va), Some(vb)) => (va - vb).abs() < EPSILON,
        (None, None) => true,
        _ => false,
    }
}

/// Helper for comparing slices of floats (e.g., column widths).
pub fn float_slices_fuzzy_eq(a: &[f32], b: &[f32]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    const EPSILON: f32 = 0.01;
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < EPSILON)
}