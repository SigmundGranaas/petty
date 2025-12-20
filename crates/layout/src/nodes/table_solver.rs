use petty_idf::TableColumnDefinition;
use crate::{LayoutEnvironment, LayoutError};
use petty_style::dimension::Dimension;
use std::time::Instant;

/// Trait to abstract cell properties needed for width solving.
/// This allows the solver to be decoupled from the specific `TableCellNode` struct.
pub trait TableCellInfo {
    fn colspan(&self) -> usize;
    fn measure_max_content(&self, env: &LayoutEnvironment) -> Result<f32, LayoutError>;
}

/// A dedicated solver for calculating table column widths.
pub struct TableSolver<'a> {
    env: &'a LayoutEnvironment<'a>,
    columns: &'a [TableColumnDefinition],
}

impl<'a> TableSolver<'a> {
    pub fn new(env: &'a LayoutEnvironment<'a>, columns: &'a [TableColumnDefinition]) -> Self {
        Self { env, columns }
    }

    /// Calculates column widths based on a CSS-like table layout algorithm.
    ///
    /// It handles:
    /// 1. Fixed widths (points)
    /// 2. Percentage widths (relative to `available_width` if bounded)
    /// 3. Auto widths (based on content measurement)
    pub fn resolve_widths<I, R, C>(
        &self,
        available_width: Option<f32>,
        rows: I,
    ) -> Result<Vec<f32>, LayoutError>
    where
        I: IntoIterator<Item = R>,
        R: IntoIterator<Item = C>,
        C: TableCellInfo,
    {
        let start = Instant::now();
        let mut measure_time = std::time::Duration::ZERO;

        let num_columns = self.columns.len();
        let mut widths = vec![0.0; num_columns];
        let mut auto_indices = Vec::new();
        let table_width = available_width.unwrap_or(0.0);
        let mut remaining_width = table_width;

        let is_bounded = available_width.is_some();

        // 1. Initial assignment based on column definitions
        for (i, col) in self.columns.iter().enumerate() {
            if let Some(dim) = &col.width {
                match dim {
                    Dimension::Pt(w) => {
                        widths[i] = *w;
                        remaining_width -= *w;
                    }
                    Dimension::Percent(p) => {
                        if is_bounded {
                            widths[i] = (p / 100.0) * table_width;
                            remaining_width -= widths[i];
                        } else {
                            auto_indices.push(i);
                        }
                    }
                    Dimension::Auto => auto_indices.push(i),
                }
            } else {
                auto_indices.push(i);
            }
        }
        remaining_width = remaining_width.max(0.0);

        // If no auto columns, we are done
        if auto_indices.is_empty() {
            let duration = start.elapsed();
            self.env.engine.record_perf("TableSolver::resolve_widths", duration);
            return Ok(widths);
        }

        // 2. Measure content for auto columns
        let mut preferred_widths: Vec<f32> = vec![0.0f32; num_columns];

        // Limit sampling to avoid performance cliff on massive tables
        const AUTO_LAYOUT_SAMPLE_LIMIT: usize = 100;

        let mut row_count = 0;
        for row in rows {
            if row_count >= AUTO_LAYOUT_SAMPLE_LIMIT {
                break;
            }
            row_count += 1;

            let mut col_cursor = 0;
            for cell in row {
                if col_cursor >= num_columns {
                    break;
                }

                let colspan = cell.colspan();
                // Only measure if this cell spans an auto column
                let involves_auto_col = (col_cursor..(col_cursor + colspan))
                    .any(|idx| auto_indices.contains(&idx));

                if involves_auto_col {
                    let m_start = Instant::now();
                    let preferred = cell.measure_max_content(self.env)?;
                    measure_time += m_start.elapsed();

                    // Simple strategy: if colspan=1, this cell dictates minimum width for this column.
                    // (Spanning cells are harder to attribute, simplistic approach ignores them for min-width)
                    if colspan == 1 {
                        preferred_widths[col_cursor] = preferred_widths[col_cursor].max(preferred);
                    }
                }
                col_cursor += colspan;
            }
        }

        // 3. Distribute remaining space
        let total_preferred: f32 = auto_indices.iter().map(|&i| preferred_widths[i]).sum();

        if !is_bounded {
            // Unbounded: Just use preferred widths for auto columns
            for &i in &auto_indices {
                widths[i] = preferred_widths[i];
            }
        } else {
            // Bounded: Distribute remaining_width based on preference
            if total_preferred > 0.0 {
                if remaining_width >= total_preferred {
                    // Expand: Distribute extra space proportionally
                    let extra_space = remaining_width - total_preferred;
                    for &i in &auto_indices {
                        widths[i] = preferred_widths[i] + extra_space * (preferred_widths[i] / total_preferred);
                    }
                } else {
                    // Shrink: Scale down proportionally to fit
                    let shrink_factor = remaining_width / total_preferred;
                    for &i in &auto_indices {
                        widths[i] = preferred_widths[i] * shrink_factor;
                    }
                }
            } else {
                // No preferred width (all empty or spanning), distribute remaining space evenly
                let width_per_auto = remaining_width / auto_indices.len() as f32;
                for i in auto_indices {
                    widths[i] = width_per_auto;
                }
            }
        }

        let duration = start.elapsed();
        self.env.engine.record_perf("TableSolver::resolve_widths", duration);
        self.env.engine.record_perf("TableSolver::resolve_widths::measure_content", measure_time);

        Ok(widths)
    }
}