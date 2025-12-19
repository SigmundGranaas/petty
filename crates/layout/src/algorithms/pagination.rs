use petty_types::geometry::Rect;

pub struct BreakAnalysis {
    pub should_break: bool,
    pub remaining_height: f32,
}

/// Centralized logic to check if a child fits in the remaining space.
///
/// * `cursor_y`: The current Y position relative to the top of the container bounds.
/// * `child_height`: The required height for the child.
/// * `bounds`: The bounds of the current container.
pub fn check_child_fit(
    cursor_y: f32,
    child_height: f32,
    bounds: Rect
) -> BreakAnalysis {
    let available = (bounds.height - cursor_y).max(0.0);
    // Use a small epsilon to handle floating point inaccuracies
    const EPSILON: f32 = 0.01;
    BreakAnalysis {
        should_break: child_height > available + EPSILON,
        remaining_height: available,
    }
}