use crate::table::table::Table;

use rayon::prelude::*;
use std::cmp;

#[derive(Debug, Clone)]
pub struct ColumnWidths {
    pub(crate) col_widths: Vec<usize>,
    pub max_col_width: usize,
    col_widths_dirty: bool
}

impl ColumnWidths {
    pub fn new() -> Self {
        Self {
            col_widths: Vec::new(),
            col_widths_dirty: true,
            max_col_width: 30
        }
    }

    /// Force recompute of column widths
    /// Uses parallel processing for large tables
    pub fn recompute(&mut self, table: &Table, parallel: bool) {
        if parallel {
            // Parallel: compute each column's max width in parallel
            // Collect all cells first to enable parallel iteration
            self.col_widths = (0..table.total_rows)
                .into_par_iter()
                .fold(|| vec![0; table.col_count()],
                      |mut acc : Vec<usize>, row_idx : usize |  {
                        if let Some(row) = table.get_row(row_idx) {
                            for (acc_w, x) in acc.iter_mut().zip(row.iter()) {
                                *acc_w = cmp::max(*acc_w, crate::util::display_width(x));
                            }
                        }

                        acc
                }).reduce(
                    || vec![0; table.col_count()],
                    |a,b| {
                        a.iter().zip(b.iter())
                            .map(|(x,y)| cmp::max(*x, *y)).collect()
                    }
                ).into_iter().map(|x| cmp::min(x, self.max_col_width)).collect();
        } else {
            // Sequential for small tables
            self.col_widths = (0..table.total_rows)
                .fold(vec![0; table.col_count()],
                      |mut acc : Vec<usize>, row_idx : usize |  {
                        if let Some(row) = table.get_row(row_idx) {
                            for (acc_w, x) in acc.iter_mut().zip(row.iter()) {
                                *acc_w = cmp::max(*acc_w, crate::util::display_width(x));
                            }
                        }

                        acc
                }).into_iter().map(|x| cmp::min(x, self.max_col_width)).collect();
        }
        self.col_widths_dirty = false;
    } 

    pub fn get_col_width(&mut self, col_idx: usize) -> usize {
        self.col_widths[col_idx]
    }

    pub fn col_widths(&mut self, table: &Table, parallel: bool) -> &[usize] {
        if self.col_widths_dirty {
            self.recompute(table, parallel);
        }
        &self.col_widths
    }

    pub fn mark_widths_dirty(&mut self) {
        self.col_widths_dirty = true;
    }

    pub fn update_col_width(&mut self, col: usize, new_len: usize) {
        if col < self.col_widths.len() {
            self.col_widths[col] = cmp::min(self.col_widths[col].max(new_len).max(3), self.max_col_width)
        }
    }

    pub fn insert_at(&mut self, idx: usize, col_size: usize) {
        if idx <= self.col_widths.len() {
            self.col_widths.insert(idx, col_size);
        }
    }

    pub fn remove_at(&mut self, idx: usize) {
        // Remove the column width entry
        if idx < self.col_widths.len() {
            self.col_widths.remove(idx);
        }
    }

    pub fn ensure_size(&mut self, cols: usize, col_size: usize) {
        while self.col_widths.len() < cols {
            self.col_widths.push(col_size);
        }
    }

    pub fn apply_permutation(&mut self, permutation: &[usize]) {
        // Reorder column widths to match
        let old_widths = self.col_widths.clone();
        for (new_col, &src_col) in permutation.iter().enumerate() {
            if new_col < self.col_widths.len() && src_col < old_widths.len() {
                self.col_widths[new_col] = old_widths[src_col];
            }
        }
    }
}
