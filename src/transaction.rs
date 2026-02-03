use crate::table::table::Table;
use crate::table::rowmanager::FilterState;

/// Represents a reversible operation on the table
#[derive(Debug, Clone)]
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
        }
    }

    /// Compute the inverse of a permutation
    /// If perm[i] = j, then inverse[j] = i
    fn inverse_permutation(perm: &[usize]) -> Vec<usize> {
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
        }
    }
}

/// Manages undo/redo history
#[derive(Debug, Default)]
pub struct History {
    undo_stack: Vec<Transaction>,
    redo_stack: Vec<Transaction>,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Record a transaction (clears redo stack)
    pub fn record(&mut self, txn: Transaction) {
        self.undo_stack.push(txn);
        self.redo_stack.clear();
    }

    /// Undo the last transaction, returns the inverse for application
    pub fn undo(&mut self) -> Option<Transaction> {
        self.undo_stack.pop().map(|txn| {
            let inverse = txn.inverse();
            self.redo_stack.push(txn);
            inverse
        })
    }

    /// Redo the last undone transaction
    pub fn redo(&mut self) -> Option<Transaction> {
        self.redo_stack.pop().map(|txn| {
            self.undo_stack.push(txn.clone());
            txn
        })
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Peek at the next undo transaction without removing it
    pub fn peek_undo(&self) -> Option<&Transaction> {
        self.undo_stack.last()
    }

    /// Peek at the next redo transaction without removing it
    pub fn peek_redo(&self) -> Option<&Transaction> {
        self.redo_stack.last()
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(rows: usize, cols: usize) -> Table {
        Table::new(vec![vec![String::new(); cols]; rows])
    }

    fn make_table_with_data(data: Vec<Vec<&str>>) -> Table {
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

    /// Helper to get a cell value for assertion comparisons
    fn cell(table: &Table, r: usize, c: usize) -> String {
        table.get_cell(r, c).unwrap().clone()
    }

    // === SetCell tests ===

    #[test]
    fn test_set_cell_apply() {
        let mut table = make_table(3, 3);
        let txn = Transaction::SetCell {
            row: 1,
            col: 1,
            old_value: String::new(),
            new_value: "hello".to_string(),
        };

        txn.apply(&mut table);
        assert_eq!(cell(&table, 1, 1), "hello");
    }

    #[test]
    fn test_set_cell_inverse() {
        let mut table = make_table(3, 3);
        table.set_cell(1, 1, "hello".to_string());

        let txn = Transaction::SetCell {
            row: 1,
            col: 1,
            old_value: String::new(),
            new_value: "hello".to_string(),
        };

        let inverse = txn.inverse();
        inverse.apply(&mut table);
        assert_eq!(cell(&table, 1, 1), "");
    }

    #[test]
    fn test_set_cell_roundtrip() {
        let mut table = make_table(3, 3);
        let original = table.clone_all_rows();

        let txn = Transaction::SetCell {
            row: 1,
            col: 1,
            old_value: String::new(),
            new_value: "hello".to_string(),
        };

        txn.apply(&mut table);
        assert_ne!(table.clone_all_rows(), original);

        txn.inverse().apply(&mut table);
        assert_eq!(table.clone_all_rows(), original);
    }

    // === InsertRow tests ===

    #[test]
    fn test_insert_row_apply() {
        let mut table = make_table(2, 3);
        let txn = Transaction::InsertRow { idx: 1 };

        txn.apply(&mut table);
        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 1), vec!["", "", ""]);
    }

    #[test]
    fn test_insert_row_at_start() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);
        let txn = Transaction::InsertRow { idx: 0 };

        txn.apply(&mut table);
        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["", ""]);
        assert_eq!(row(&table, 1), vec!["a", "b"]);
    }

    #[test]
    fn test_insert_row_at_end() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);
        let txn = Transaction::InsertRow { idx: 2 };

        txn.apply(&mut table);
        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 2), vec!["", ""]);
    }

    // === DeleteRow tests ===

    #[test]
    fn test_delete_row_apply() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b"],
            vec!["c", "d"],
            vec!["e", "f"],
        ]);
        let txn = Transaction::DeleteRow {
            idx: 1,
            data: vec!["c".to_string(), "d".to_string()],
        };

        txn.apply(&mut table);
        assert_eq!(table.row_count(), 2);
        assert_eq!(row(&table, 0), vec!["a", "b"]);
        assert_eq!(row(&table, 1), vec!["e", "f"]);
    }

    #[test]
    fn test_delete_row_inverse_restores() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b"],
            vec!["e", "f"],
        ]);
        let txn = Transaction::DeleteRow {
            idx: 1,
            data: vec!["c".to_string(), "d".to_string()],
        };

        // The inverse of delete is insert with data
        let inverse = txn.inverse();
        inverse.apply(&mut table);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 1), vec!["c", "d"]);
    }

    // === InsertCol tests ===

    #[test]
    fn test_insert_col_apply() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);
        let txn = Transaction::InsertCol { idx: 1 };

        txn.apply(&mut table);
        assert_eq!(row(&table, 0), vec!["a", "", "b"]);
        assert_eq!(row(&table, 1), vec!["c", "", "d"]);
    }

    // === DeleteCol tests ===

    #[test]
    fn test_delete_col_apply() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);
        let txn = Transaction::DeleteCol {
            idx: 1,
            data: vec!["b".to_string(), "e".to_string()],
        };

        txn.apply(&mut table);
        assert_eq!(row(&table, 0), vec!["a", "c"]);
        assert_eq!(row(&table, 1), vec!["d", "f"]);
    }

    #[test]
    fn test_delete_col_inverse_restores() {
        let mut table = make_table_with_data(vec![
            vec!["a", "c"],
            vec!["d", "f"],
        ]);
        let txn = Transaction::DeleteCol {
            idx: 1,
            data: vec!["b".to_string(), "e".to_string()],
        };

        let inverse = txn.inverse();
        inverse.apply(&mut table);

        assert_eq!(row(&table, 0), vec!["a", "b", "c"]);
        assert_eq!(row(&table, 1), vec!["d", "e", "f"]);
    }

    // === SetSpan tests ===

    #[test]
    fn test_set_span_apply() {
        let mut table = make_table(3, 3);
        let txn = Transaction::SetSpan {
            row: 0,
            col: 0,
            old_data: vec![vec!["".to_string(); 2]; 2],
            new_data: vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ],
        };

        txn.apply(&mut table);
        assert_eq!(cell(&table, 0, 0), "a");
        assert_eq!(cell(&table, 0, 1), "b");
        assert_eq!(cell(&table, 1, 0), "c");
        assert_eq!(cell(&table, 1, 1), "d");
        assert_eq!(cell(&table, 2, 2), ""); // Unchanged
    }

    #[test]
    fn test_set_span_inverse() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b", "x"],
            vec!["c", "d", "x"],
            vec!["x", "x", "x"],
        ]);

        let txn = Transaction::SetSpan {
            row: 0,
            col: 0,
            old_data: vec![
                vec!["".to_string(), "".to_string()],
                vec!["".to_string(), "".to_string()],
            ],
            new_data: vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ],
        };

        let inverse = txn.inverse();
        inverse.apply(&mut table);

        assert_eq!(cell(&table, 0, 0), "");
        assert_eq!(cell(&table, 0, 1), "");
        assert_eq!(cell(&table, 1, 0), "");
        assert_eq!(cell(&table, 1, 1), "");
        assert_eq!(cell(&table, 2, 2), "x"); // Unchanged
    }

    // === Batch tests ===

    #[test]
    fn test_batch_apply() {
        let mut table = make_table(3, 3);
        let txn = Transaction::Batch(vec![
            Transaction::SetCell {
                row: 0,
                col: 0,
                old_value: String::new(),
                new_value: "a".to_string(),
            },
            Transaction::SetCell {
                row: 1,
                col: 1,
                old_value: String::new(),
                new_value: "b".to_string(),
            },
        ]);

        txn.apply(&mut table);
        assert_eq!(cell(&table, 0, 0), "a");
        assert_eq!(cell(&table, 1, 1), "b");
    }

    #[test]
    fn test_batch_inverse_reverses_order() {
        let mut table = make_table_with_data(vec![
            vec!["a", ""],
            vec!["", "b"],
        ]);

        let txn = Transaction::Batch(vec![
            Transaction::SetCell {
                row: 0,
                col: 0,
                old_value: String::new(),
                new_value: "a".to_string(),
            },
            Transaction::SetCell {
                row: 1,
                col: 1,
                old_value: String::new(),
                new_value: "b".to_string(),
            },
        ]);

        let inverse = txn.inverse();
        inverse.apply(&mut table);

        assert_eq!(cell(&table, 0, 0), "");
        assert_eq!(cell(&table, 1, 1), "");
    }

    // === History tests ===

    #[test]
    fn test_history_record_and_undo() {
        let mut history = History::new();
        let mut table = make_table(3, 3);

        let txn = Transaction::SetCell {
            row: 0,
            col: 0,
            old_value: String::new(),
            new_value: "hello".to_string(),
        };

        txn.apply(&mut table);
        history.record(txn);

        assert_eq!(cell(&table, 0, 0), "hello");

        if let Some(undo) = history.undo() {
            undo.apply(&mut table);
        }

        assert_eq!(cell(&table, 0, 0), "");
    }

    #[test]
    fn test_history_redo() {
        let mut history = History::new();
        let mut table = make_table(3, 3);

        let txn = Transaction::SetCell {
            row: 0,
            col: 0,
            old_value: String::new(),
            new_value: "hello".to_string(),
        };

        txn.apply(&mut table);
        history.record(txn);

        // Undo
        if let Some(undo) = history.undo() {
            undo.apply(&mut table);
        }
        assert_eq!(cell(&table, 0, 0), "");

        // Redo
        if let Some(redo) = history.redo() {
            redo.apply(&mut table);
        }
        assert_eq!(cell(&table, 0, 0), "hello");
    }

    #[test]
    fn test_history_new_action_clears_redo() {
        let mut history = History::new();
        let mut table = make_table(3, 3);

        // First action
        let txn1 = Transaction::SetCell {
            row: 0,
            col: 0,
            old_value: String::new(),
            new_value: "first".to_string(),
        };
        txn1.apply(&mut table);
        history.record(txn1);

        // Undo
        if let Some(undo) = history.undo() {
            undo.apply(&mut table);
        }

        assert!(history.can_redo());

        // New action should clear redo stack
        let txn2 = Transaction::SetCell {
            row: 1,
            col: 1,
            old_value: String::new(),
            new_value: "second".to_string(),
        };
        txn2.apply(&mut table);
        history.record(txn2);

        assert!(!history.can_redo());
    }

    #[test]
    fn test_history_multiple_undos() {
        let mut history = History::new();
        let mut table = make_table(3, 3);

        for i in 0..5 {
            let txn = Transaction::SetCell {
                row: 0,
                col: 0,
                old_value: if i == 0 { String::new() } else { (i - 1).to_string() },
                new_value: i.to_string(),
            };
            txn.apply(&mut table);
            history.record(txn);
        }

        assert_eq!(cell(&table, 0, 0), "4");

        // Undo all
        for expected in (0..4).rev() {
            if let Some(undo) = history.undo() {
                undo.apply(&mut table);
            }
            assert_eq!(cell(&table, 0, 0), expected.to_string());
        }

        // One more undo to get back to empty
        if let Some(undo) = history.undo() {
            undo.apply(&mut table);
        }
        assert_eq!(cell(&table, 0, 0), "");

        // No more undos
        assert!(!history.can_undo());
    }

    #[test]
    fn test_history_can_undo_can_redo() {
        let mut history = History::new();

        assert!(!history.can_undo());
        assert!(!history.can_redo());

        history.record(Transaction::SetCell {
            row: 0,
            col: 0,
            old_value: String::new(),
            new_value: "x".to_string(),
        });

        assert!(history.can_undo());
        assert!(!history.can_redo());

        history.undo();

        assert!(!history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn test_history_clear() {
        let mut history = History::new();

        history.record(Transaction::SetCell {
            row: 0,
            col: 0,
            old_value: String::new(),
            new_value: "x".to_string(),
        });

        history.undo();
        assert!(history.can_redo());

        history.record(Transaction::SetCell {
            row: 0,
            col: 0,
            old_value: String::new(),
            new_value: "y".to_string(),
        });
        assert!(history.can_undo());

        history.clear();

        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    // === InsertRowWithData / InsertColWithData tests ===

    #[test]
    fn test_insert_row_with_data() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let txn = Transaction::InsertRowWithData {
            idx: 1,
            data: vec!["x".to_string(), "y".to_string()],
        };

        txn.apply(&mut table);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["a", "b"]);
        assert_eq!(row(&table, 1), vec!["x", "y"]);
        assert_eq!(row(&table, 2), vec!["c", "d"]);
    }

    #[test]
    fn test_insert_col_with_data() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let txn = Transaction::InsertColWithData {
            idx: 1,
            data: vec!["x".to_string(), "y".to_string()],
        };

        txn.apply(&mut table);

        assert_eq!(row(&table, 0), vec!["a", "x", "b"]);
        assert_eq!(row(&table, 1), vec!["c", "y", "d"]);
    }

    // === Complex scenario tests ===

    #[test]
    fn test_complex_undo_redo_sequence() {
        let mut history = History::new();
        let mut table = make_table(3, 3);

        // Insert a row
        let txn1 = Transaction::InsertRow { idx: 1 };
        txn1.apply(&mut table);
        // Manually capture what was inserted for proper undo
        let row_data = row(&table, 1);
        history.record(Transaction::InsertRowWithData { idx: 1, data: row_data });

        assert_eq!(table.row_count(), 4);

        // Set some cells
        let txn2 = Transaction::SetCell {
            row: 1,
            col: 0,
            old_value: String::new(),
            new_value: "inserted".to_string(),
        };
        txn2.apply(&mut table);
        history.record(txn2);

        assert_eq!(cell(&table, 1, 0), "inserted");

        // Undo the cell change
        if let Some(undo) = history.undo() {
            undo.apply(&mut table);
        }
        assert_eq!(cell(&table, 1, 0), "");

        // Undo the row insert
        if let Some(undo) = history.undo() {
            undo.apply(&mut table);
        }
        assert_eq!(table.row_count(), 3);

        // Redo row insert
        if let Some(redo) = history.redo() {
            redo.apply(&mut table);
        }
        assert_eq!(table.row_count(), 4);

        // Redo cell change
        if let Some(redo) = history.redo() {
            redo.apply(&mut table);
        }
        assert_eq!(cell(&table, 1, 0), "inserted");
    }

    // === PermuteRows tests ===

    #[test]
    fn test_permute_rows_apply() {
        let mut table = make_table_with_data(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
        ]);

        // Reverse the rows: [2, 1, 0] means row 0 <- old row 2, etc.
        let txn = Transaction::PermuteRows {
            permutation: vec![2, 1, 0],
        };

        txn.apply(&mut table);

        assert_eq!(row(&table, 0), vec!["c", "3"]);
        assert_eq!(row(&table, 1), vec!["b", "2"]);
        assert_eq!(row(&table, 2), vec!["a", "1"]);
    }

    #[test]
    fn test_permute_rows_inverse() {
        let mut table = make_table_with_data(vec![
            vec!["c", "3"],
            vec!["b", "2"],
            vec!["a", "1"],
        ]);

        // The permutation that created this state
        let txn = Transaction::PermuteRows {
            permutation: vec![2, 1, 0],
        };

        // Apply the inverse to restore original order
        let inverse = txn.inverse();
        inverse.apply(&mut table);

        assert_eq!(row(&table, 0), vec!["a", "1"]);
        assert_eq!(row(&table, 1), vec!["b", "2"]);
        assert_eq!(row(&table, 2), vec!["c", "3"]);
    }

    #[test]
    fn test_permute_rows_roundtrip() {
        let mut table = make_table_with_data(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
            vec!["d", "4"],
        ]);
        let original = table.clone_all_rows();

        // Shuffle: [3, 0, 2, 1] means new order is [d, a, c, b]
        let txn = Transaction::PermuteRows {
            permutation: vec![3, 0, 2, 1],
        };

        txn.apply(&mut table);
        assert_ne!(table.clone_all_rows(), original);

        // Undo should restore original
        txn.inverse().apply(&mut table);
        assert_eq!(table.clone_all_rows(), original);
    }

    #[test]
    fn test_permute_rows_large_scale() {
        // Create a large table (3000 rows, spans 3 chunks)
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("{}", i)])
            .collect();
        let mut table = Table::new(rows);

        // Create reverse permutation
        let permutation: Vec<usize> = (0..3000).rev().collect();
        let txn = Transaction::PermuteRows { permutation };

        txn.apply(&mut table);

        // Verify reversed
        assert_eq!(table.get_cell(0, 0), Some(&"2999".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"0".to_string()));

        // Undo
        txn.inverse().apply(&mut table);

        // Verify restored
        assert_eq!(table.get_cell(0, 0), Some(&"0".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"2999".to_string()));
    }

    #[test]
    fn test_permute_cols_apply() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b", "c"],
            vec!["1", "2", "3"],
        ]);

        // Reverse the columns
        let txn = Transaction::PermuteCols {
            permutation: vec![2, 1, 0],
        };

        txn.apply(&mut table);

        assert_eq!(row(&table, 0), vec!["c", "b", "a"]);
        assert_eq!(row(&table, 1), vec!["3", "2", "1"]);
    }

    #[test]
    fn test_permute_cols_roundtrip() {
        let mut table = make_table_with_data(vec![
            vec!["a", "b", "c"],
            vec!["1", "2", "3"],
        ]);
        let original = table.clone_all_rows();

        let txn = Transaction::PermuteCols {
            permutation: vec![2, 0, 1],
        };

        txn.apply(&mut table);
        assert_ne!(table.clone_all_rows(), original);

        txn.inverse().apply(&mut table);
        assert_eq!(table.clone_all_rows(), original);
    }

    #[test]
    fn test_inverse_permutation_correctness() {
        // Test the inverse_permutation helper directly
        let perm = vec![3, 0, 2, 1]; // Maps: 0<-3, 1<-0, 2<-2, 3<-1
        let inv = Transaction::inverse_permutation(&perm);

        // Inverse should satisfy: inv[perm[i]] = i
        for i in 0..perm.len() {
            assert_eq!(inv[perm[i]], i);
        }

        // Also: perm[inv[i]] = i
        for i in 0..inv.len() {
            assert_eq!(perm[inv[i]], i);
        }
    }
}
