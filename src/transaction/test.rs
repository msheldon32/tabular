use super::transaction::*;
use super::history::*;
use super::clipboard::*;

use crate::table::table::Table;


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

#[test]
fn test_clipboard_new() {
    let clipboard = Clipboard::new();
    assert!(clipboard.unnamed.is_none());
    assert!(clipboard.yank_register.is_none());
    assert!(clipboard.selected.is_none());
}

#[test]
fn test_paste_as_transaction_nothing() {
    let mut clipboard = Clipboard::new();
    let table = make_table_with_data(vec![vec!["a"]]);

    let (msg, txn) = clipboard.paste_as_transaction(0, 0, &table);

    assert_eq!(msg, "Nothing to paste");
    assert!(txn.is_none());
}

#[test]
fn test_select_invalid_register() {
    let mut clipboard = Clipboard::new();
    assert!(clipboard.select_register('!').is_err());
    assert!(clipboard.select_register('1').is_err()); // Only 0 is valid number
}

#[test]
fn test_select_valid_registers() {
    let mut clipboard = Clipboard::new();
    assert!(clipboard.select_register('a').is_ok());
    assert!(clipboard.select_register('z').is_ok());
    assert!(clipboard.select_register('A').is_ok()); // Uppercase treated as lowercase
    assert!(clipboard.select_register('0').is_ok());
    assert!(clipboard.select_register('_').is_ok());
    assert!(clipboard.select_register('+').is_ok());
    assert!(clipboard.select_register('"').is_ok());
}
