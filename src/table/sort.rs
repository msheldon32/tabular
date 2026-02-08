//! Sorting functionality for Table

use rayon::prelude::*;

use crate::numeric::format::parse_numeric;
use crate::util::ColumnType;
use super::table::{Table, CHUNK_SIZE};

/// Threshold for using parallel processing
const PARALLEL_THRESHOLD: usize = 10_000;

/// Sorting direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Maximum cells to sample for type detection
const TYPE_PROBE_SAMPLE_SIZE: usize = 20;

impl Table {
    /// Probe a column to determine if it's numeric or text
    /// Samples up to TYPE_PROBE_SAMPLE_SIZE non-empty cells for efficiency
    /// Recognizes formatted numbers (currency, percentages, etc.)
    pub fn probe_column_type(&self, col: usize, skip_header: bool) -> ColumnType {
        let start_row = if skip_header { 1 } else { 0 };
        let mut numeric_count = 0;
        let mut total_count = 0;

        for row_idx in start_row..self.row_count() {
            if total_count >= TYPE_PROBE_SAMPLE_SIZE {
                break;
            }
            if let Some(cell) = self.get_cell(row_idx, col) {
                let trimmed = cell.trim();
                if !trimmed.is_empty() {
                    total_count += 1;
                    if parse_numeric(trimmed).is_some() {
                        numeric_count += 1;
                    }
                }
            }
        }

        // If more than half are numeric (or all are numeric), treat as numeric
        if total_count > 0 && numeric_count * 2 >= total_count {
            ColumnType::Numeric
        } else {
            ColumnType::Text
        }
    }

    /// Probe a row to determine if it's numeric or text
    /// Samples up to TYPE_PROBE_SAMPLE_SIZE non-empty cells for efficiency
    /// Recognizes formatted numbers (currency, percentages, etc.)
    pub fn probe_row_type(&self, row: usize, skip_first_col: bool) -> ColumnType {
        let start_col = if skip_first_col { 1 } else { 0 };
        let mut numeric_count = 0;
        let mut total_count = 0;

        if let Some(row_data) = self.get_row(row) {
            for col_idx in start_col..row_data.len() {
                if total_count >= TYPE_PROBE_SAMPLE_SIZE {
                    break;
                }
                let trimmed = row_data[col_idx].trim();
                if !trimmed.is_empty() {
                    total_count += 1;
                    if parse_numeric(trimmed).is_some() {
                        numeric_count += 1;
                    }
                }
            }
        }

        if total_count > 0 && numeric_count * 2 >= total_count {
            ColumnType::Numeric
        } else {
            ColumnType::Text
        }
    }

    /// Sort rows by a specific column, returns the sorted indices
    /// skip_header: if true, row 0 is not included in sorting
    /// Uses parallel processing for large tables
    pub fn get_sorted_row_indices(
        &self,
        sort_col: usize,
        direction: SortDirection,
        skip_header: bool,
    ) -> Vec<usize> {
        let sort_type = self.probe_column_type(sort_col, skip_header);
        let start_row = if skip_header { 1 } else { 0 };
        let row_count = self.row_count();
        let use_parallel = row_count >= PARALLEL_THRESHOLD;

        let mut indices: Vec<usize> = if skip_header {
            vec![0]
        } else {
            Vec::new()
        };

        match sort_type {
            ColumnType::Numeric => {
                // Build keyed vector (parallel for large tables)
                let mut keyed: Vec<(usize, f64)> = if use_parallel {
                    // Need to collect cell references first for parallel access
                    let cells: Vec<Option<&String>> = (start_row..row_count)
                        .map(|row| self.get_cell(row, sort_col))
                        .collect();

                    cells.into_par_iter()
                        .enumerate()
                        .map(|(i, cell)| {
                            let row = start_row + i;
                            let val = parse_numeric(
                                cell.map(|x| x.as_str().trim()).unwrap_or("")
                            ).unwrap_or(f64::NAN);
                            (row, val)
                        })
                        .collect()
                } else {
                    (start_row..row_count)
                        .map(|row| {
                            let val = parse_numeric(
                                self.get_cell(row, sort_col).map(|x| x.as_str().trim()).unwrap_or("")
                            ).unwrap_or(f64::NAN);
                            (row, val)
                        })
                        .collect()
                };

                // Sort (parallel for large tables)
                let cmp_fn = |&(idx_a, num_a): &(usize, f64), &(idx_b, num_b): &(usize, f64)| -> std::cmp::Ordering {
                    let base = match (num_a.is_nan(), num_b.is_nan()) {
                        (true, true) => std::cmp::Ordering::Equal,
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        (false, false) => num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal),
                    };
                    match direction {
                        SortDirection::Ascending => base.then(idx_a.cmp(&idx_b)),
                        SortDirection::Descending => base.reverse().then(idx_a.cmp(&idx_b)),
                    }
                };

                if use_parallel {
                    keyed.par_sort_unstable_by(cmp_fn);
                } else {
                    keyed.sort_unstable_by(cmp_fn);
                }

                indices.extend(keyed.into_iter().map(|(row, _)| row));
            }
            ColumnType::Text => {
                // Build keyed vector (parallel for large tables)
                let mut keyed: Vec<(usize, String)> = if use_parallel {
                    let cells: Vec<Option<&String>> = (start_row..row_count)
                        .map(|row| self.get_cell(row, sort_col))
                        .collect();

                    cells.into_par_iter()
                        .enumerate()
                        .map(|(i, cell)| {
                            let row = start_row + i;
                            let val = cell.map(|s| s.to_lowercase().trim().to_owned())
                                .unwrap_or_default();
                            (row, val)
                        })
                        .collect()
                } else {
                    (start_row..row_count)
                        .map(|row| {
                            let val = self.get_cell(row, sort_col)
                                .map(|s| s.to_lowercase().trim().to_owned())
                                .unwrap_or_default();
                            (row, val)
                        })
                        .collect()
                };

                // Sort (parallel for large tables)
                let cmp_fn = |&(ref i, ref a): &(usize, String), &(ref j, ref b): &(usize, String)| -> std::cmp::Ordering {
                    match direction {
                        SortDirection::Ascending => a.cmp(b).then(i.cmp(j)),
                        SortDirection::Descending => a.cmp(b).reverse().then(i.cmp(j)),
                    }
                };

                if use_parallel {
                    keyed.par_sort_unstable_by(cmp_fn);
                } else {
                    keyed.sort_unstable_by(cmp_fn);
                }

                indices.extend(keyed.into_iter().map(|(row, _)| row));
            }
        }

        indices
    }

    /// Sort columns by a specific row, returns the sorted column indices
    /// Uses parallel processing for tables with many columns
    pub fn get_sorted_col_indices(
        &self,
        sort_row: usize,
        direction: SortDirection,
        skip_first_col: bool,
    ) -> Vec<usize> {
        let sort_type = self.probe_row_type(sort_row, skip_first_col);
        let start_col = if skip_first_col { 1 } else { 0 };
        let col_count = self.col_count();
        let use_parallel = col_count >= PARALLEL_THRESHOLD;

        let mut indices: Vec<usize> = if skip_first_col {
            vec![0]
        } else {
            Vec::new()
        };

        match sort_type {
            ColumnType::Numeric => {
                let mut keyed: Vec<(usize, f64)> = if use_parallel {
                    let cells: Vec<Option<&String>> = (start_col..col_count)
                        .map(|col| self.get_cell(sort_row, col))
                        .collect();

                    cells.into_par_iter()
                        .enumerate()
                        .map(|(i, cell)| {
                            let col = start_col + i;
                            let val = parse_numeric(
                                cell.map(|x| x.as_str().trim()).unwrap_or("")
                            ).unwrap_or(f64::NAN);
                            (col, val)
                        })
                        .collect()
                } else {
                    (start_col..col_count)
                        .map(|col| {
                            let val = parse_numeric(
                                self.get_cell(sort_row, col).map(|x| x.as_str().trim()).unwrap_or("")
                            ).unwrap_or(f64::NAN);
                            (col, val)
                        })
                        .collect()
                };

                let cmp_fn = |&(idx_a, num_a): &(usize, f64), &(idx_b, num_b): &(usize, f64)| -> std::cmp::Ordering {
                    let base = match (num_a.is_nan(), num_b.is_nan()) {
                        (true, true) => std::cmp::Ordering::Equal,
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        (false, false) => num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal),
                    };
                    match direction {
                        SortDirection::Ascending => base.then(idx_a.cmp(&idx_b)),
                        SortDirection::Descending => base.reverse().then(idx_a.cmp(&idx_b)),
                    }
                };

                if use_parallel {
                    keyed.par_sort_unstable_by(cmp_fn);
                } else {
                    keyed.sort_unstable_by(cmp_fn);
                }

                indices.extend(keyed.into_iter().map(|(col, _)| col));
            }
            ColumnType::Text => {
                let mut keyed: Vec<(usize, String)> = if use_parallel {
                    let cells: Vec<Option<&String>> = (start_col..col_count)
                        .map(|col| self.get_cell(sort_row, col))
                        .collect();

                    cells.into_par_iter()
                        .enumerate()
                        .map(|(i, cell)| {
                            let col = start_col + i;
                            let val = cell.map(|s| s.to_lowercase().trim().to_owned())
                                .unwrap_or_default();
                            (col, val)
                        })
                        .collect()
                } else {
                    (start_col..col_count)
                        .map(|col| {
                            let val = self.get_cell(sort_row, col)
                                .map(|s| s.to_lowercase().trim().to_owned())
                                .unwrap_or_default();
                            (col, val)
                        })
                        .collect()
                };

                let cmp_fn = |&(ref i, ref a): &(usize, String), &(ref j, ref b): &(usize, String)| -> std::cmp::Ordering {
                    match direction {
                        SortDirection::Ascending => a.cmp(b).then(i.cmp(j)),
                        SortDirection::Descending => a.cmp(b).reverse().then(i.cmp(j)),
                    }
                };

                if use_parallel {
                    keyed.par_sort_unstable_by(cmp_fn);
                } else {
                    keyed.sort_unstable_by(cmp_fn);
                }

                indices.extend(keyed.into_iter().map(|(col, _)| col));
            }
        }

        indices
    }

    /// Apply a row permutation in-place (memory-efficient)
    /// permutation[i] = j means row i in new table comes from row j in old table
    pub fn apply_row_permutation(&mut self, permutation: &[usize]) {
        if permutation.len() != self.row_count() {
            return;
        }

        // Flatten chunks, apply permutation, rechunk
        let old_chunks = std::mem::take(&mut self.chunks);
        let mut flat_rows: Vec<Vec<String>> = old_chunks.into_iter().flatten().collect();

        // Build new order by taking rows according to permutation
        let mut new_rows = Vec::with_capacity(flat_rows.len());
        for &src_idx in permutation {
            if src_idx < flat_rows.len() {
                new_rows.push(std::mem::take(&mut flat_rows[src_idx]));
            }
        }

        // Rechunk
        self.chunks = new_rows
            .chunks(CHUNK_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();
        self.total_rows = self.chunks.iter().map(|c| c.len()).sum();
        self.mark_widths_dirty();
    }

    /// Apply a column permutation in-place
    /// permutation[i] = j means column i in new table comes from column j in old table
    pub fn apply_col_permutation(&mut self, permutation: &[usize]) {
        if permutation.len() != self.col_count() {
            return;
        }

        for chunk in &mut self.chunks {
            for row in chunk {
                let old_row = row.clone();
                for (new_col, &src_col) in permutation.iter().enumerate() {
                    if new_col < row.len() && src_col < old_row.len() {
                        row[new_col] = old_row[src_col].clone();
                    }
                }
            }
        }
        
        self.col_widths.lock().unwrap().apply_permutation(permutation);
    }

    /// Get the permutation needed to sort rows by a column
    /// Returns None if already sorted
    pub fn get_sort_permutation(
        &self,
        sort_col: usize,
        direction: SortDirection,
        skip_header: bool,
    ) -> Option<Vec<usize>> {
        let new_order = self.get_sorted_row_indices(sort_col, direction, skip_header);

        // Check if already sorted
        if new_order.iter().enumerate().all(|(i, &idx)| i == idx) {
            return None;
        }

        Some(new_order)
    }

    /// Get the permutation needed to sort columns by a row
    /// Returns None if already sorted
    pub fn get_col_sort_permutation(
        &self,
        sort_row: usize,
        direction: SortDirection,
        skip_first_col: bool,
    ) -> Option<Vec<usize>> {
        let new_order = self.get_sorted_col_indices(sort_row, direction, skip_first_col);

        // Check if already sorted
        if new_order.iter().enumerate().all(|(i, &idx)| i == idx) {
            return None;
        }

        Some(new_order)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(data: Vec<Vec<&str>>) -> Table {
        Table::new(
            data.into_iter()
                .map(|row| row.into_iter().map(|s| s.to_string()).collect())
                .collect()
        )
    }

    /// Helper to get a row as Vec<String> for assertion comparisons
    fn row(table: &Table, idx: usize) -> Vec<String> {
        table.get_row(idx).unwrap().to_vec()
    }

    /// Helper to get a cell value as &str for assertion comparisons
    fn cell(table: &Table, r: usize, c: usize) -> String {
        table.get_cell(r, c).unwrap().clone()
    }

    // === Table basic operations ===

    #[test]
    fn test_table_new() {
        let table = Table::new(vec![vec!["".to_string()]]);
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.col_count(), 1);
        assert_eq!(cell(&table, 0, 0), "");
    }

    #[test]
    fn test_table_get_cell() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_cell(0, 0), Some(&"a".to_string()));
        assert_eq!(table.get_cell(0, 1), Some(&"b".to_string()));
        assert_eq!(table.get_cell(1, 0), Some(&"c".to_string()));
        assert_eq!(table.get_cell(1, 1), Some(&"d".to_string()));
        assert_eq!(table.get_cell(2, 0), None);
        assert_eq!(table.get_cell(0, 2), None);
    }

    #[test]
    fn test_table_set_cell() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.set_cell(0, 1, "x".to_string());
        assert_eq!(cell(&table, 0, 1), "x");

        // Out of bounds should not panic
        table.set_cell(10, 10, "y".to_string());
    }

    #[test]
    fn test_table_row_col_count() {
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);

        assert_eq!(table.row_count(), 2);
        assert_eq!(table.col_count(), 3);
    }

    // === Row operations ===

    #[test]
    fn test_insert_row_at_beginning() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(0);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["", ""]);
        assert_eq!(row(&table, 1), vec!["a", "b"]);
    }

    #[test]
    fn test_insert_row_at_middle() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(1);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["a", "b"]);
        assert_eq!(row(&table, 1), vec!["", ""]);
        assert_eq!(row(&table, 2), vec!["c", "d"]);
    }

    #[test]
    fn test_insert_row_at_end() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(2);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 2), vec!["", ""]);
    }

    #[test]
    fn test_delete_row_at() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
            vec!["e", "f"],
        ]);

        let deleted = table.delete_row_at(1);

        assert_eq!(deleted, Some(vec!["c".to_string(), "d".to_string()]));
        assert_eq!(table.row_count(), 2);
        assert_eq!(row(&table, 0), vec!["a", "b"]);
        assert_eq!(row(&table, 1), vec!["e", "f"]);
    }

    #[test]
    fn test_delete_last_row_clears_instead() {
        let mut table = make_table(vec![
            vec!["a", "b"],
        ]);

        let deleted = table.delete_row_at(0);

        assert_eq!(deleted, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.row_count(), 1);
        assert_eq!(row(&table, 0), vec!["", ""]);
    }

    #[test]
    fn test_get_row() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_row(0).map(|r| r.to_vec()), Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.get_row(1).map(|r| r.to_vec()), Some(vec!["c".to_string(), "d".to_string()]));
        assert_eq!(table.get_row(2), None);
    }

    // === Column operations ===

    #[test]
    fn test_insert_col_at_beginning() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_at(0);

        assert_eq!(table.col_count(), 3);
        assert_eq!(row(&table, 0), vec!["", "a", "b"]);
        assert_eq!(row(&table, 1), vec!["", "c", "d"]);
    }

    #[test]
    fn test_insert_col_at_middle() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_at(1);

        assert_eq!(table.col_count(), 3);
        assert_eq!(row(&table, 0), vec!["a", "", "b"]);
        assert_eq!(row(&table, 1), vec!["c", "", "d"]);
    }

    #[test]
    fn test_delete_col_at() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);

        let deleted = table.delete_col_at(1);

        assert_eq!(deleted, Some(vec!["b".to_string(), "e".to_string()]));
        assert_eq!(table.col_count(), 2);
        assert_eq!(row(&table, 0), vec!["a", "c"]);
        assert_eq!(row(&table, 1), vec!["d", "f"]);
    }

    #[test]
    fn test_delete_last_col_clears_instead() {
        let mut table = make_table(vec![
            vec!["a"],
            vec!["b"],
        ]);

        let deleted = table.delete_col_at(0);

        assert_eq!(deleted, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.col_count(), 1);
        assert_eq!(row(&table, 0), vec![""]);
        assert_eq!(row(&table, 1), vec![""]);
    }

    #[test]
    fn test_get_col() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_col_cloned(0), Some(vec!["a".to_string(), "c".to_string()]));
        assert_eq!(table.get_col_cloned(1), Some(vec!["b".to_string(), "d".to_string()]));
        assert_eq!(table.get_col_cloned(2), None);
    }

    // === Span operations ===

    #[test]
    fn test_get_span() {
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        let span = table.get_span(0, 1, 0, 1).unwrap();
        assert_eq!(span, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["d".to_string(), "e".to_string()],
        ]);
    }

    #[test]
    fn test_get_span_single_cell() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let span = table.get_span(0, 0, 0, 0).unwrap();
        assert_eq!(span, vec![vec!["a".to_string()]]);
    }

    #[test]
    fn test_get_span_full_table() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let span = table.get_span(0, 1, 0, 1).unwrap();
        assert_eq!(span, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string(), "d".to_string()],
        ]);
    }

    #[test]
    fn test_get_span_out_of_bounds() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        // Request span that extends beyond table bounds
        let span = table.get_span(0, 3, 0, 3).unwrap();
        assert_eq!(span.len(), 4); // 4 rows requested
        assert_eq!(span[0].len(), 4); // 4 cols requested
        // Valid cells have values, out of bounds are empty
        assert_eq!(span[0][0], "a");
        assert_eq!(span[0][1], "b");
        assert_eq!(span[0][2], ""); // out of bounds
        assert_eq!(span[2][0], ""); // out of bounds
    }

    #[test]
    fn test_ensure_size() {
        let mut table = make_table(vec![
            vec!["a", "b"],
        ]);

        assert_eq!(table.row_count(), 1);
        assert_eq!(table.col_count(), 2);

        table.ensure_size(3, 4);

        assert_eq!(table.row_count(), 3);
        assert_eq!(table.col_count(), 4);
        assert_eq!(cell(&table, 0, 0), "a");
        assert_eq!(cell(&table, 0, 3), "");
        assert_eq!(cell(&table, 2, 0), "");
    }

    // === Insert with data ===

    #[test]
    fn test_insert_row_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_with_data(1, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 1), vec!["x", "y"]);
    }

    #[test]
    fn test_insert_row_with_data_pads_short_row() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
        ]);

        table.insert_row_with_data(1, vec!["x".to_string()]);

        assert_eq!(row(&table, 1), vec!["x", "", ""]);
    }

    #[test]
    fn test_insert_col_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_with_data(1, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(row(&table, 0), vec!["a", "x", "b"]);
        assert_eq!(row(&table, 1), vec!["c", "y", "d"]);
    }
}
