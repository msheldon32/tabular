use crate::table::table::Table;
use crate::table::rowmanager::FilterState;

/// Represents a reversible operation on the table
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Transaction {
    /// Set a single cell value
    SetCell {
        row: usize,
        col: usize,
        old_value: String,
        new_value: String,
    },
    /// Insert an empty row at index
    InsertRow { idx: usize },
    /// Insert a row with data at index
    InsertRowWithData { idx: usize, data: Vec<String> },
    /// Delete a row (stores data for undo)
    DeleteRow { idx: usize, data: Vec<String> },
    /// Insert multiple empty rows at index
    InsertRowsBulk { idx: usize, count: usize },
    /// Insert multiple rows with data at index
    InsertRowsWithDataBulk { idx: usize, data: Vec<Vec<String>> },
    /// Delete multiple contiguous rows (stores data for undo)
    DeleteRowsBulk { idx: usize, data: Vec<Vec<String>> },
    /// Insert an empty column at index
    InsertCol { idx: usize },
    /// Insert a column with data at index
    InsertColWithData { idx: usize, data: Vec<String> },
    /// Delete a column (stores data for undo)
    DeleteCol { idx: usize, data: Vec<String> },
    /// Set multiple cells in a rectangular region
    SetSpan {
        row: usize,
        col: usize,
        old_data: Vec<Vec<String>>,
        new_data: Vec<Vec<String>>,
    },
    /// Reorder rows by permutation (memory-efficient for sorting)
    /// permutation[i] = j means row i in new table comes from row j in old table
    PermuteRows { permutation: Vec<usize> },
    /// Reorder columns by permutation (memory-efficient for sorting)
    PermuteCols { permutation: Vec<usize> },
    /// Change filter state (stores old and new state for undo/redo)
    /// Note: This transaction does NOT modify the table; app.rs handles
    /// applying filter state to RowManager separately.
    SetFilter { old_state: FilterState, new_state: FilterState },
    /// Multiple transactions grouped together
    Batch(Vec<Transaction>),
    Undo,
    Redo
}

impl Transaction {
    /// Estimate the size/complexity of this transaction for progress reporting
    /// Returns the number of cells or operations involved
    pub fn estimated_size(&self) -> usize {
        match self {
            Transaction::SetCell { .. } => 1,
            Transaction::InsertRow { .. } => 1,
            Transaction::InsertRowWithData { data, .. } => data.len(),
            Transaction::DeleteRow { data, .. } => data.len(),
            Transaction::InsertRowsBulk { count, .. } => *count,
            Transaction::InsertRowsWithDataBulk { data, .. } => {
                data.iter().map(|r| r.len()).sum::<usize>().max(data.len())
            }
            Transaction::DeleteRowsBulk { data, .. } => {
                data.iter().map(|r| r.len()).sum::<usize>().max(data.len())
            }
            Transaction::InsertCol { .. } => 1,
            Transaction::InsertColWithData { data, .. } => data.len(),
            Transaction::DeleteCol { data, .. } => data.len(),
            Transaction::SetSpan { new_data, .. } => {
                new_data.iter().map(|r| r.len()).sum()
            }
            Transaction::PermuteRows { permutation } => permutation.len(),
            Transaction::PermuteCols { permutation } => permutation.len(),
            Transaction::SetFilter { .. } => 1, // Filter changes are instant
            Transaction::Batch(txns) => txns.iter().map(|t| t.estimated_size()).sum(),
            Transaction::Undo => 1,
            Transaction::Redo => 1
        }
    }

    /// Check if this transaction is large enough to warrant progress display
    pub fn is_large(&self) -> bool {
        self.estimated_size() >= 50_000
    }

    /// If this is a SetFilter transaction, returns the new filter state to apply
    pub fn filter_state(&self) -> Option<&FilterState> {
        match self {
            Transaction::SetFilter { new_state, .. } => Some(new_state),
            _ => None,
        }
    }

    pub fn apply(&self, table: &mut Table) {
        match self {
            Transaction::SetCell { row, col, new_value, .. } => {
                table.set_cell(*row, *col, new_value.clone());
                table.recompute_col_widths();
            }
            Transaction::InsertRow { idx } => {
                table.insert_row_at(*idx);
            }
            Transaction::InsertRowWithData { idx, data } => {
                table.insert_row_with_data(*idx, data.clone());
                table.recompute_col_widths();
            }
            Transaction::DeleteRow { idx, .. } => {
                table.delete_row_at(*idx);
                table.recompute_col_widths();
            }
            Transaction::InsertRowsBulk { idx, count } => {
                table.insert_rows_bulk(*idx, *count);
            }
            Transaction::InsertRowsWithDataBulk { idx, data } => {
                table.insert_rows_with_data_bulk(*idx, data.clone());
                table.recompute_col_widths();
            }
            Transaction::DeleteRowsBulk { idx, data } => {
                table.delete_rows_bulk(*idx, data.len());
                table.recompute_col_widths();
            }
            Transaction::InsertCol { idx } => {
                table.insert_col_at(*idx);
            }
            Transaction::InsertColWithData { idx, data } => {
                table.insert_col_with_data(*idx, data.clone());
            }
            Transaction::DeleteCol { idx, .. } => {
                table.delete_col_at(*idx);
            }
            Transaction::SetSpan { row, col, new_data, .. } => {
                // Ensure table is large enough for the span
                let needed_rows = row + new_data.len();
                let needed_cols = col + new_data.first().map(|r| r.len()).unwrap_or(0);
                table.ensure_size(needed_rows, needed_cols);

                for (dr, row_data) in new_data.iter().enumerate() {
                    for (dc, value) in row_data.iter().enumerate() {
                        table.set_cell(row + dr, col + dc, value.clone());
                    }
                }
                table.recompute_col_widths();
            }
            Transaction::PermuteRows { permutation } => {
                table.apply_row_permutation(permutation);
            }
            Transaction::PermuteCols { permutation } => {
                table.apply_col_permutation(permutation);
            }
            Transaction::SetFilter { .. } => {
                // Filter state is not stored in the table; app.rs handles
                // applying filter state to RowManager when this transaction
                // is applied or inverted.
            }
            Transaction::Batch(txns) => {
                for txn in txns {
                    txn.apply(table);
                }
            }
            Transaction::Undo => {
                // handled directly by app for now
            }
            Transaction::Redo => {
                // handled directly by app for now
            }
        }
    }

    /// Compute the inverse of a permutation
    /// If perm[i] = j, then inverse[j] = i
    pub fn inverse_permutation(perm: &[usize]) -> Vec<usize> {
        let mut inv = vec![0; perm.len()];
        for (i, &p) in perm.iter().enumerate() {
            if p < inv.len() {
                inv[p] = i;
            }
        }
        inv
    }

    pub fn inverse(&self) -> Transaction {
        match self {
            Transaction::SetCell { row, col, old_value, new_value } => {
                Transaction::SetCell {
                    row: *row,
                    col: *col,
                    old_value: new_value.clone(),
                    new_value: old_value.clone(),
                }
            }
            Transaction::InsertRow { idx } => {
                Transaction::DeleteRow { idx: *idx, data: Vec::new() }
            }
            Transaction::InsertRowWithData { idx, data } => {
                Transaction::DeleteRow { idx: *idx, data: data.clone() }
            }
            Transaction::DeleteRow { idx, data } => {
                Transaction::InsertRowWithData { idx: *idx, data: data.clone() }
            }
            Transaction::InsertRowsBulk { idx, count } => {
                // To undo, we need to delete the rows (but we don't have their data)
                // This is only correct for empty rows
                Transaction::DeleteRowsBulk { idx: *idx, data: vec![Vec::new(); *count] }
            }
            Transaction::InsertRowsWithDataBulk { idx, data } => {
                Transaction::DeleteRowsBulk { idx: *idx, data: data.clone() }
            }
            Transaction::DeleteRowsBulk { idx, data } => {
                Transaction::InsertRowsWithDataBulk { idx: *idx, data: data.clone() }
            }
            Transaction::InsertCol { idx } => {
                Transaction::DeleteCol { idx: *idx, data: Vec::new() }
            }
            Transaction::InsertColWithData { idx, data } => {
                Transaction::DeleteCol { idx: *idx, data: data.clone() }
            }
            Transaction::DeleteCol { idx, data } => {
                Transaction::InsertColWithData { idx: *idx, data: data.clone() }
            }
            Transaction::SetSpan { row, col, old_data, new_data } => {
                Transaction::SetSpan {
                    row: *row,
                    col: *col,
                    old_data: new_data.clone(),
                    new_data: old_data.clone(),
                }
            }
            Transaction::PermuteRows { permutation } => {
                Transaction::PermuteRows {
                    permutation: Self::inverse_permutation(permutation),
                }
            }
            Transaction::PermuteCols { permutation } => {
                Transaction::PermuteCols {
                    permutation: Self::inverse_permutation(permutation),
                }
            }
            Transaction::SetFilter { old_state, new_state } => {
                Transaction::SetFilter {
                    old_state: new_state.clone(),
                    new_state: old_state.clone(),
                }
            }
            Transaction::Batch(txns) => {
                Transaction::Batch(txns.iter().rev().map(|t| t.inverse()).collect())
            }
            // These are a bit nonsensical but I'm leaving these in. These cannot be assigned to
            // history anyways
            Transaction::Undo => { Transaction::Redo }
            Transaction::Redo => { Transaction::Undo }
        }
    }
}
