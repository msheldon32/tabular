

pub struct ColumnWidths {
    pub(crate) col_widths: Vec<usize>,
    pub max_col_width: usize,
    col_widths_dirty: bool
}

impl ColumnWidths {
    /// Force recompute of column widths
    /// Uses parallel processing for large tables
    pub fn recompute_col_widths(&mut self, rows_iter: Iterator<Item = &Vec<String>>) {
        let size = self.total_rows * self.col_count;

        if size >= PARALLEL_THRESHOLD && self.col_count > 1 {
            // Parallel: compute each column's max width in parallel
            // Collect all cells first to enable parallel iteration
            let all_rows: Vec<&Vec<String>> = self.rows_iter().collect();

            self.col_widths = (0..self.col_count)
                .into_par_iter()
                .map(|col| {
                    all_rows
                        .iter()
                        .filter_map(|row| row.get(col))
                        .map(|s| crate::util::display_width(s))
                        .max()
                        .unwrap_or(3)
                        .max(3)
                        .min(self.max_col_width)
                })
                .collect();
        } else {
            // Sequential for small tables
            self.col_widths = (0..self.col_count)
                .map(|col| {
                    self.rows_iter()
                        .filter_map(|row| row.get(col))
                        .map(|s| crate::util::display_width(s))
                        .max()
                        .unwrap_or(3)
                        .max(3)
                        .min(self.max_col_width)
                })
                .collect();
        }
        self.col_widths_dirty = false;
    }
    
}
