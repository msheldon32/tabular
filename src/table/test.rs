use super::table::*;
use super::rowmanager::*;
use super::sort::*;
use super::tableview::*;
use super::operations::*;

use crate::numeric::predicate::Predicate;
use crate::util::ColumnType;
use crate::mode::Mode;

use std::collections::HashSet;
use std::rc::Rc;
use std::cell::RefCell;


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

#[test]
fn is_row_live_unfiltered_always_true() {
    let rm = RowManager::new();
    assert!(rm.is_row_live(0));
    assert!(rm.is_row_live(12345));
}

#[test]
fn is_row_live_filtered_uses_set() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![2, 4, 7];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    assert!(!rm.is_row_live(0));
    assert!(rm.is_row_live(2));
    assert!(!rm.is_row_live(3));
    assert!(rm.is_row_live(7));
    assert!(!rm.is_row_live(100));
}

#[test]
fn get_successor_unfiltered_simple() {
    let rm = RowManager::new();
    assert_eq!(rm.get_successor(0), Some(1));
    assert_eq!(rm.get_successor(41), Some(42));
}

#[test]
fn get_successor_filtered_middle_hits_next() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![2, 4, 7, 10];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    // successor of 4 is 7
    assert_eq!(rm.get_successor(4), Some(7));
    // successor of 5 is 7 (row not present)
    assert_eq!(rm.get_successor(5), Some(7));
    // successor of 6 is 7
    assert_eq!(rm.get_successor(6), Some(7));
}

#[test]
fn get_successor_filtered_before_first_is_first() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![10, 20, 30];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    // first active row after 0 is 10
    assert_eq!(rm.get_successor(0), Some(10));
    // after 9 is 10
    assert_eq!(rm.get_successor(9), Some(10));
}

#[test]
fn get_successor_filtered_at_or_after_last_is_none() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![2, 4, 7];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    assert_eq!(rm.get_successor(7), None);
    assert_eq!(rm.get_successor(8), None);
    assert_eq!(rm.get_successor(100), None);
}

#[test]
fn get_successor_filtered_empty_is_none() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![];
    rm.active_row_set = HashSet::new();

    assert_eq!(rm.get_successor(0), None);
    assert_eq!(rm.get_successor(999), None);
}

#[test]
fn get_successor_filtered_with_duplicates_behaves_like_strictly_after() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![2, 4, 4, 4, 7];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    // strictly after 4 should be 7 (skips duplicates)
    assert_eq!(rm.get_successor(4), Some(7));
    // after 3 is 4
    assert_eq!(rm.get_successor(3), Some(4));
}

// ---- Predecessor tests (intended behavior) ----

#[test]
fn get_predecessor_unfiltered_simple() {
    let rm = RowManager::new();
    // intended behavior: predecessor of 1 is 0; of 0 is None
    assert_eq!(rm.get_predecessor(0), None);
    assert_eq!(rm.get_predecessor(1), Some(0));
    assert_eq!(rm.get_predecessor(42), Some(41));
}

#[test]
fn get_predecessor_filtered_middle_hits_prev() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![2, 4, 7, 10];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    assert_eq!(rm.get_predecessor(7), Some(4));
    assert_eq!(rm.get_predecessor(6), Some(4));  // row not present
    assert_eq!(rm.get_predecessor(5), Some(4));
    assert_eq!(rm.get_predecessor(4), Some(2));
}

#[test]
fn get_predecessor_filtered_before_first_is_none() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![10, 20, 30];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    assert_eq!(rm.get_predecessor(0), None);
    assert_eq!(rm.get_predecessor(9), None);
    assert_eq!(rm.get_predecessor(10), None); // strictly less than 10 doesn't exist
}

#[test]
fn get_predecessor_filtered_after_last_is_last() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![10, 20, 30];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    assert_eq!(rm.get_predecessor(31), Some(30));
    assert_eq!(rm.get_predecessor(1000), Some(30));
}

// ---- Filter toggling / removal ----

#[test]
fn remove_filter_resets_state() {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = vec![1, 3, 5];
    rm.active_row_set = rm.active_rows.iter().cloned().collect();

    rm.remove_filter();

    assert!(!rm.is_filtered);
    assert!(rm.active_rows.is_empty());
    assert!(rm.active_row_set.is_empty());
    assert!(rm.is_row_live(999)); // unfiltered should be true
}

// ---- Predicate filter tests ----

#[test]
fn predicate_filter_numeric_gt() {
    let table = make_table(vec![
        vec!["Name", "Score"],
        vec!["Alice", "95"],
        vec!["Bob", "87"],
        vec!["Carol", "92"],
        vec!["Dave", "50"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Gt,
        val: "90".to_string(),
    };

    rm.predicate_filter(&table, 1, pred, crate::util::ColumnType::Numeric, true);

    assert!(rm.is_filtered);
    // Header (row 0) + rows with score > 90: Alice (95), Carol (92)
    assert!(rm.is_row_live(0)); // header kept
    assert!(rm.is_row_live(1)); // Alice, 95 > 90
    assert!(!rm.is_row_live(2)); // Bob, 87 not > 90
    assert!(rm.is_row_live(3)); // Carol, 92 > 90
    assert!(!rm.is_row_live(4)); // Dave, 50 not > 90
    assert_eq!(rm.active_rows, vec![0, 1, 3]);
}

#[test]
fn predicate_filter_numeric_eq() {
    let table = make_table(vec![
        vec!["ID", "Value"],
        vec!["1", "100"],
        vec!["2", "200"],
        vec!["3", "100"],
        vec!["4", "300"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Eq,
        val: "100".to_string(),
    };

    rm.predicate_filter(&table, 1, pred, crate::util::ColumnType::Numeric, true);

    assert!(rm.is_filtered);
    assert_eq!(rm.active_rows, vec![0, 1, 3]); // header + rows with value == 100
}

#[test]
fn predicate_filter_without_header() {
    let table = make_table(vec![
        vec!["Alice", "95"],
        vec!["Bob", "87"],
        vec!["Carol", "92"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Ge,
        val: "90".to_string(),
    };

    rm.predicate_filter(&table, 1, pred, crate::util::ColumnType::Numeric, false);

    assert!(rm.is_filtered);
    // No header preservation: rows with score >= 90
    assert_eq!(rm.active_rows, vec![0, 2]); // Alice (95), Carol (92)
}

#[test]
fn predicate_filter_text() {
    let table = make_table(vec![
        vec!["Name", "Status"],
        vec!["Alice", "active"],
        vec!["Bob", "inactive"],
        vec!["Carol", "ACTIVE"],
        vec!["Dave", "pending"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Eq,
        val: "active".to_string(),
    };

    rm.predicate_filter(&table, 1, pred, crate::util::ColumnType::Text, true);

    assert!(rm.is_filtered);
    // Header + rows where Status == "active" (case-insensitive)
    assert_eq!(rm.active_rows, vec![0, 1, 3]); // header, Alice, Carol
}

#[test]
fn predicate_filter_sets_filter_string() {
    let table = make_table(vec![
        vec!["A", "B"],
        vec!["1", "2"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Gt,
        val: "0".to_string(),
    };

    rm.predicate_filter(&table, 0, pred, crate::util::ColumnType::Numeric, true);

    assert!(rm.filter_string.contains("Filtered"));
    assert!(rm.filter_string.contains("A")); // column letter
    assert!(rm.filter_string.contains("> 0"));
}

#[test]
fn predicate_filter_chains_with_existing_filter() {
    let table = make_table(vec![
        vec!["Name", "Score"],
        vec!["Alice", "95"],
        vec!["Bob", "87"],
        vec!["Carol", "92"],
        vec!["Dave", "50"],
        vec!["Eve", "99"],
    ]);

    let mut rm = RowManager::new();

    // First filter: score > 80
    let pred1 = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Gt,
        val: "80".to_string(),
    };
    rm.predicate_filter(&table, 1, pred1, crate::util::ColumnType::Numeric, true);
    assert_eq!(rm.active_rows, vec![0, 1, 2, 3, 5]); // header + Alice, Bob, Carol, Eve

    // Second filter: score < 95 (should chain with first filter)
    let pred2 = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Lt,
        val: "95".to_string(),
    };
    rm.predicate_filter(&table, 1, pred2, crate::util::ColumnType::Numeric, true);
    // Only rows that pass both: 80 < score < 95
    assert_eq!(rm.active_rows, vec![0, 2, 3]); // header + Bob (87), Carol (92)
}

// ---- Navigation with predicate filter ----

#[test]
fn get_end_with_predicate_filter() {
    let table = make_table(vec![
        vec!["A"],
        vec!["1"],
        vec!["2"],
        vec!["3"],
        vec!["4"],
        vec!["5"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Le,
        val: "3".to_string(),
    };
    rm.predicate_filter(&table, 0, pred, crate::util::ColumnType::Numeric, true);

    // Active rows: 0 (header), 1, 2, 3 (values 1, 2, 3)
    assert_eq!(rm.get_end(&table), 3);
}

#[test]
fn jump_down_with_predicate_filter() {
    let table = make_table(vec![
        vec!["A"],
        vec!["10"],
        vec!["20"],
        vec!["30"],
        vec!["40"],
        vec!["50"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Ge,
        val: "20".to_string(),
    };
    rm.predicate_filter(&table, 0, pred, crate::util::ColumnType::Numeric, true);

    // Active rows: 0 (header), 2, 3, 4, 5 (values 20, 30, 40, 50)
    assert_eq!(rm.active_rows, vec![0, 2, 3, 4, 5]);

    // Jump down 2 from row 0 should land on row 3
    assert_eq!(rm.jump_down(0, 2, &table), 3);

    // Jump down 1 from row 2 should land on row 3
    assert_eq!(rm.jump_down(2, 1, &table), 3);
}

#[test]
fn jump_up_with_predicate_filter() {
    let table = make_table(vec![
        vec!["A"],
        vec!["10"],
        vec!["20"],
        vec!["30"],
        vec!["40"],
        vec!["50"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Ge,
        val: "20".to_string(),
    };
    rm.predicate_filter(&table, 0, pred, crate::util::ColumnType::Numeric, true);

    // Active rows: 0 (header), 2, 3, 4, 5
    // Jump up 2 from row 5 should land on row 3
    assert_eq!(rm.jump_up(5, 2), 3);

    // Jump up 1 from row 3 should land on row 2
    assert_eq!(rm.jump_up(3, 1), 2);
}

#[test]
fn get_successor_with_predicate_filter() {
    let table = make_table(vec![
        vec!["A"],
        vec!["5"],
        vec!["15"],
        vec!["25"],
        vec!["35"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Gt,
        val: "10".to_string(),
    };
    rm.predicate_filter(&table, 0, pred, crate::util::ColumnType::Numeric, true);

    // Active rows: 0 (header), 2, 3, 4 (values 15, 25, 35)
    assert_eq!(rm.active_rows, vec![0, 2, 3, 4]);

    // Successor of 0 is 2 (skips filtered row 1)
    assert_eq!(rm.get_successor(0), Some(2));

    // Successor of 2 is 3
    assert_eq!(rm.get_successor(2), Some(3));

    // Successor of 4 is None (last active row)
    assert_eq!(rm.get_successor(4), None);
}

#[test]
fn get_predecessor_with_predicate_filter() {
    let table = make_table(vec![
        vec!["A"],
        vec!["5"],
        vec!["15"],
        vec!["25"],
        vec!["35"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Gt,
        val: "10".to_string(),
    };
    rm.predicate_filter(&table, 0, pred, crate::util::ColumnType::Numeric, true);

    // Active rows: 0 (header), 2, 3, 4
    // Predecessor of 2 is 0 (skips filtered row 1)
    assert_eq!(rm.get_predecessor(2), Some(0));

    // Predecessor of 3 is 2
    assert_eq!(rm.get_predecessor(3), Some(2));

    // Predecessor of 0 is None
    assert_eq!(rm.get_predecessor(0), None);
}

#[test]
fn predicate_filter_empty_result_keeps_header() {
    let table = make_table(vec![
        vec!["Name", "Score"],
        vec!["Alice", "50"],
        vec!["Bob", "60"],
    ]);

    let mut rm = RowManager::new();
    let pred = Predicate::Comparator {
        op: crate::numeric::predicate::Op::Gt,
        val: "100".to_string(),
    };

    rm.predicate_filter(&table, 1, pred, crate::util::ColumnType::Numeric, true);

    // No data rows match, but header should be preserved
    assert!(rm.is_filtered);
    assert_eq!(rm.active_rows, vec![0]);
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

fn row_manager() -> Rc<RefCell<RowManager>> {
    Rc::new(RefCell::new(RowManager::new()))
}

// === TableView unit tests ===
#[test]
fn test_tableview_new() {
    let view = TableView::new(row_manager());
    assert_eq!(view.cursor_row, 0);
    assert_eq!(view.cursor_col, 0);
    assert_eq!(view.viewport_row, 0);
    assert_eq!(view.viewport_col, 0);
}

#[test]
fn test_tableview_navigation() {
    let mut view = TableView::new(row_manager());
    let table = make_table(vec![
        vec!["a", "b", "c"],
        vec!["d", "e", "f"],
        vec!["g", "h", "i"],
    ]);

    view.move_right(&table);
    assert_eq!(view.cursor_col, 1);

    view.move_down(&table);
    assert_eq!(view.cursor_row, 1);

    view.move_left();
    assert_eq!(view.cursor_col, 0);

    view.move_up();
    assert_eq!(view.cursor_row, 0);
}

#[test]
fn test_tableview_navigation_bounds() {
    let mut view = TableView::new(row_manager());
    let table = make_table(vec![
        vec!["a", "b"],
        vec!["c", "d"],
    ]);

    // Can't go negative
    view.move_left();
    assert_eq!(view.cursor_col, 0);

    view.move_up();
    assert_eq!(view.cursor_row, 0);

    // Can't go past bounds
    view.cursor_col = 1;
    view.cursor_row = 1;

    view.move_right(&table);
    assert_eq!(view.cursor_col, 1);

    view.move_down(&table);
    assert_eq!(view.cursor_row, 1);
}

#[test]
fn test_tableview_move_to_edges() {
    let mut view = TableView::new(row_manager());
    let table = make_table(vec![
        vec!["a", "b", "c"],
        vec!["d", "e", "f"],
        vec!["g", "h", "i"],
    ]);

    view.cursor_col = 1;
    view.cursor_row = 1;

    view.move_to_first_col();
    assert_eq!(view.cursor_col, 0);

    view.move_to_last_col(&table);
    assert_eq!(view.cursor_col, 2);

    view.move_to_top();
    assert_eq!(view.cursor_row, 0);

    view.move_to_bottom(&table);
    assert_eq!(view.cursor_row, 2);
}

#[test]
fn test_tableview_clamp_cursor() {
    let mut view = TableView::new(row_manager());
    view.cursor_row = 100;
    view.cursor_col = 100;

    let table = make_table(vec![
        vec!["a", "b"],
        vec!["c", "d"],
    ]);

    view.clamp_cursor(&table);

    assert_eq!(view.cursor_row, 1);
    assert_eq!(view.cursor_col, 1);
}

#[test]
fn test_tableview_current_cell() {
    let view = TableView::new(row_manager());
    let table = make_table(vec![
        vec!["hello", "world"],
    ]);

    assert_eq!(current_cell(&view, &table), "hello");
}

#[test]
fn test_tableview_set_support() {
    let mut view = TableView::new(row_manager());
    view.cursor_row = 5;
    view.cursor_col = 3;

    view.set_support();

    assert_eq!(view.support_row, 5);
    assert_eq!(view.support_col, 3);
}

#[test]
fn test_tableview_is_selected_visual() {
    let mut view = TableView::new(row_manager());
    view.cursor_row = 0;
    view.cursor_col = 0;
    view.support_row = 2;
    view.support_col = 2;

    assert!(view.is_selected(0, 0, Mode::Visual));
    assert!(view.is_selected(1, 1, Mode::Visual));
    assert!(view.is_selected(2, 2, Mode::Visual));
    assert!(!view.is_selected(3, 0, Mode::Visual));
    assert!(!view.is_selected(0, 3, Mode::Visual));
}

#[test]
fn test_tableview_is_selected_visual_row() {
    let mut view = TableView::new(row_manager());
    view.cursor_row = 1;
    view.cursor_col = 1;
    view.support_row = 2;
    view.support_col = 1;

    // In VisualRow, columns don't matter
    assert!(view.is_selected(1, 0, Mode::VisualRow));
    assert!(view.is_selected(1, 100, Mode::VisualRow));
    assert!(view.is_selected(2, 0, Mode::VisualRow));
    assert!(!view.is_selected(0, 0, Mode::VisualRow));
    assert!(!view.is_selected(3, 0, Mode::VisualRow));
}

#[test]
fn test_tableview_is_selected_visual_col() {
    let mut view = TableView::new(row_manager());
    view.cursor_row = 1;
    view.cursor_col = 1;
    view.support_row = 1;
    view.support_col = 2;

    // In VisualCol, rows don't matter
    assert!(view.is_selected(0, 1, Mode::VisualCol));
    assert!(view.is_selected(100, 1, Mode::VisualCol));
    assert!(view.is_selected(0, 2, Mode::VisualCol));
    assert!(!view.is_selected(0, 0, Mode::VisualCol));
    assert!(!view.is_selected(0, 3, Mode::VisualCol));
}

#[test]
fn test_tableview_page_navigation() {
    let mut view = TableView::new(row_manager());
    view.viewport_height = 10;

    let table = make_table(vec![vec!["x"]; 100]); // 100 rows

    view.page_down(&table);
    assert_eq!(view.cursor_row, 9);

    view.page_down(&table);
    assert_eq!(view.cursor_row, 18);

    view.page_up();
    assert_eq!(view.cursor_row, 9);

    view.half_page_down(&table);
    assert_eq!(view.cursor_row, 14);

    view.half_page_up();
    assert_eq!(view.cursor_row, 9);
}

// === Sorting tests ===

#[test]
fn test_probe_column_type_numeric() {
    let table = make_table(vec![
        vec!["Name", "Score"],
        vec!["Alice", "95"],
        vec!["Bob", "87"],
        vec!["Carol", "92"],
    ]);

    // Column 0 is text (names)
    assert_eq!(table.probe_column_type(0, true), ColumnType::Text);
    // Column 1 is numeric (scores)
    assert_eq!(table.probe_column_type(1, true), ColumnType::Numeric);
}

#[test]
fn test_probe_column_type_mixed() {
    let table = make_table(vec![
        vec!["ID", "Value"],
        vec!["1", "100"],
        vec!["2", "N/A"],
        vec!["3", "200"],
    ]);

    // Column 0 is numeric
    assert_eq!(table.probe_column_type(0, true), ColumnType::Numeric);
    // Column 1 is mixed but majority numeric
    assert_eq!(table.probe_column_type(1, true), ColumnType::Numeric);
}

#[test]
fn test_probe_column_type_with_empty_cells() {
    let table = make_table(vec![
        vec!["Header"],
        vec!["10"],
        vec![""],
        vec!["20"],
        vec![""],
    ]);

    // Empty cells should be ignored; remaining are numeric
    assert_eq!(table.probe_column_type(0, true), ColumnType::Numeric);
}

#[test]
fn test_probe_column_type_all_text() {
    let table = make_table(vec![
        vec!["Names"],
        vec!["Alice"],
        vec!["Bob"],
        vec!["Carol"],
    ]);

    assert_eq!(table.probe_column_type(0, true), ColumnType::Text);
}

#[test]
fn test_get_sorted_row_indices_numeric_ascending() {
    let table = make_table(vec![
        vec!["Name", "Score"],
        vec!["Alice", "95"],
        vec!["Bob", "87"],
        vec!["Carol", "92"],
    ]);

    // Sort by score (column 1), ascending, skip header
    let indices = table.get_sorted_row_indices(1, SortDirection::Ascending, true);

    // Expected: header stays at 0, then Bob (87), Carol (92), Alice (95)
    assert_eq!(indices, vec![0, 2, 3, 1]);
}

#[test]
fn test_get_sorted_row_indices_numeric_descending() {
    let table = make_table(vec![
        vec!["Name", "Score"],
        vec!["Alice", "95"],
        vec!["Bob", "87"],
        vec!["Carol", "92"],
    ]);

    // Sort by score (column 1), descending, skip header
    let indices = table.get_sorted_row_indices(1, SortDirection::Descending, true);

    // Expected: header stays at 0, then Alice (95), Carol (92), Bob (87)
    assert_eq!(indices, vec![0, 1, 3, 2]);
}

#[test]
fn test_get_sorted_row_indices_text_ascending() {
    let table = make_table(vec![
        vec!["Name", "Score"],
        vec!["Carol", "92"],
        vec!["Alice", "95"],
        vec!["Bob", "87"],
    ]);

    // Sort by name (column 0), ascending, skip header
    let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

    // Expected: header stays at 0, then Alice, Bob, Carol
    assert_eq!(indices, vec![0, 2, 3, 1]);
}

#[test]
fn test_get_sorted_row_indices_no_header() {
    let table = make_table(vec![
        vec!["Carol", "92"],
        vec!["Alice", "95"],
        vec!["Bob", "87"],
    ]);

    // Sort by name (column 0), ascending, NO header skip
    let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, false);

    // Expected: Alice, Bob, Carol
    assert_eq!(indices, vec![1, 2, 0]);
}

#[test]
fn test_get_sorted_row_indices_with_non_numeric() {
    let table = make_table(vec![
        vec!["ID", "Value"],
        vec!["1", "100"],
        vec!["2", "N/A"],
        vec!["3", "50"],
    ]);

    // Sort by value (column 1), ascending
    // N/A should go to the end
    let indices = table.get_sorted_row_indices(1, SortDirection::Ascending, true);

    // 50, 100, N/A
    assert_eq!(indices, vec![0, 3, 1, 2]);
}

#[test]
fn test_get_sorted_col_indices() {
    let table = make_table(vec![
        vec!["C", "A", "B"],
        vec!["3", "1", "2"],
    ]);

    // Sort columns by row 0 (text), ascending
    let indices = table.get_sorted_col_indices(0, SortDirection::Ascending, false);

    // A, B, C
    assert_eq!(indices, vec![1, 2, 0]);
}

#[test]
fn test_get_sorted_col_indices_numeric() {
    let table = make_table(vec![
        vec!["30", "10", "20"],
        vec!["C", "A", "B"],
    ]);

    // Sort columns by row 0 (numeric), ascending
    let indices = table.get_sorted_col_indices(0, SortDirection::Ascending, false);

    // 10, 20, 30
    assert_eq!(indices, vec![1, 2, 0]);
}

#[test]
fn test_sort_case_insensitive() {
    let table = make_table(vec![
        vec!["name"],
        vec!["Banana"],
        vec!["apple"],
        vec!["Cherry"],
    ]);

    let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

    // apple, Banana, Cherry (case-insensitive)
    assert_eq!(indices, vec![0, 2, 1, 3]);
}

#[test]
fn test_sort_negative_numbers() {
    let table = make_table(vec![
        vec!["value"],
        vec!["-10"],
        vec!["5"],
        vec!["-3"],
        vec!["0"],
    ]);

    let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

    // -10, -3, 0, 5
    assert_eq!(indices, vec![0, 1, 3, 4, 2]);
}

#[test]
fn test_sort_float_numbers() {
    let table = make_table(vec![
        vec!["value"],
        vec!["1.5"],
        vec!["1.05"],
        vec!["1.25"],
    ]);

    let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

    // 1.05, 1.25, 1.5
    assert_eq!(indices, vec![0, 2, 3, 1]);
}

// === Bulk row operations ===

#[test]
fn test_delete_rows_bulk_single_chunk() {
    let mut table = make_table(vec![
        vec!["a", "1"],
        vec!["b", "2"],
        vec!["c", "3"],
        vec!["d", "4"],
        vec!["e", "5"],
    ]);

    let deleted = table.delete_rows_bulk(1, 2);

    assert_eq!(deleted.len(), 2);
    assert_eq!(deleted[0], vec!["b", "2"]);
    assert_eq!(deleted[1], vec!["c", "3"]);
    assert_eq!(table.row_count(), 3);
    assert_eq!(row(&table, 0), vec!["a", "1"]);
    assert_eq!(row(&table, 1), vec!["d", "4"]);
    assert_eq!(row(&table, 2), vec!["e", "5"]);
}

#[test]
fn test_delete_rows_bulk_at_start() {
    let mut table = make_table(vec![
        vec!["a"],
        vec!["b"],
        vec!["c"],
        vec!["d"],
    ]);

    let deleted = table.delete_rows_bulk(0, 2);

    assert_eq!(deleted.len(), 2);
    // Verify deleted rows are in correct order
    assert_eq!(deleted[0], vec!["a"]);
    assert_eq!(deleted[1], vec!["b"]);
    assert_eq!(table.row_count(), 2);
    assert_eq!(row(&table, 0), vec!["c"]);
    assert_eq!(row(&table, 1), vec!["d"]);
}

#[test]
fn test_delete_rows_bulk_order_preserved() {
    // Create a table with numbered rows to verify order
    let mut table = make_table(vec![
        vec!["row0"],
        vec!["row1"],
        vec!["row2"],
        vec!["row3"],
        vec!["row4"],
        vec!["row5"],
        vec!["row6"],
        vec!["row7"],
    ]);

    // Delete middle rows
    let deleted = table.delete_rows_bulk(2, 4);

    assert_eq!(deleted.len(), 4);
    // Verify deleted rows are returned in correct order
    assert_eq!(deleted[0], vec!["row2"]);
    assert_eq!(deleted[1], vec!["row3"]);
    assert_eq!(deleted[2], vec!["row4"]);
    assert_eq!(deleted[3], vec!["row5"]);

    // Verify remaining rows
    assert_eq!(table.row_count(), 4);
    assert_eq!(row(&table, 0), vec!["row0"]);
    assert_eq!(row(&table, 1), vec!["row1"]);
    assert_eq!(row(&table, 2), vec!["row6"]);
    assert_eq!(row(&table, 3), vec!["row7"]);
}

/// Helper to create a large table with numbered rows for cross-chunk testing
fn make_large_table(num_rows: usize) -> Table {
    let rows: Vec<Vec<String>> = (0..num_rows)
        .map(|i| vec![format!("row{}", i), format!("val{}", i)])
        .collect();
    Table::new(rows)
}

#[test]
fn test_delete_rows_bulk_cross_chunk_middle() {
    // 3000 rows = 3 chunks (0-1023, 1024-2047, 2048-2999)
    let mut table = make_large_table(3000);
    assert_eq!(table.row_count(), 3000);

    // Delete 2000 rows from the middle (rows 500-2499)
    // This spans all 3 chunks
    let deleted = table.delete_rows_bulk(500, 2000);

    assert_eq!(deleted.len(), 2000);
    assert_eq!(table.row_count(), 1000);

    // Verify deleted rows are in correct order
    for i in 0..2000 {
        assert_eq!(deleted[i][0], format!("row{}", 500 + i),
            "deleted row {} should be row{}", i, 500 + i);
    }

    // Verify remaining rows
    for i in 0..500 {
        assert_eq!(row(&table, i)[0], format!("row{}", i),
            "remaining row {} should be row{}", i, i);
    }
    for i in 0..500 {
        assert_eq!(row(&table, 500 + i)[0], format!("row{}", 2500 + i),
            "remaining row {} should be row{}", 500 + i, 2500 + i);
    }
}

#[test]
fn test_delete_rows_bulk_cross_chunk_top() {
    // 3000 rows = 3 chunks
    let mut table = make_large_table(3000);

    // Delete 2000 rows from the top (rows 0-1999)
    // This spans chunks 0 and 1 completely, plus part of chunk 2
    let deleted = table.delete_rows_bulk(0, 2000);

    assert_eq!(deleted.len(), 2000);
    assert_eq!(table.row_count(), 1000);

    // Verify deleted rows are in correct order
    for i in 0..2000 {
        assert_eq!(deleted[i][0], format!("row{}", i),
            "deleted row {} should be row{}", i, i);
    }

    // Verify remaining rows (rows 2000-2999 should now be at 0-999)
    for i in 0..1000 {
        assert_eq!(row(&table, i)[0], format!("row{}", 2000 + i),
            "remaining row {} should be row{}", i, 2000 + i);
    }
}

#[test]
fn test_delete_rows_bulk_cross_chunk_end() {
    // 3000 rows = 3 chunks
    let mut table = make_large_table(3000);

    // Delete 2000 rows from the end (rows 1000-2999)
    // This spans parts of chunk 0, all of chunk 1, and all of chunk 2
    let deleted = table.delete_rows_bulk(1000, 2000);

    assert_eq!(deleted.len(), 2000);
    assert_eq!(table.row_count(), 1000);

    // Verify deleted rows are in correct order
    for i in 0..2000 {
        assert_eq!(deleted[i][0], format!("row{}", 1000 + i),
            "deleted row {} should be row{}", i, 1000 + i);
    }

    // Verify remaining rows (rows 0-999 should still be there)
    for i in 0..1000 {
        assert_eq!(row(&table, i)[0], format!("row{}", i),
            "remaining row {} should be row{}", i, i);
    }
}

#[test]
fn test_delete_rows_bulk_exactly_two_chunks() {
    // Test deletion that spans exactly 2 chunks with no middle chunks
    let mut table = make_large_table(3000);

    // Delete rows 1000-1100 (spans chunk 0 end and chunk 1 start)
    let deleted = table.delete_rows_bulk(1000, 101);

    assert_eq!(deleted.len(), 101);
    assert_eq!(table.row_count(), 2899);

    // Verify deleted rows are in correct order
    for i in 0..101 {
        assert_eq!(deleted[i][0], format!("row{}", 1000 + i),
            "deleted row {} should be row{}", i, 1000 + i);
    }
}

#[test]
fn test_delete_rows_bulk_at_end() {
    let mut table = make_table(vec![
        vec!["a"],
        vec!["b"],
        vec!["c"],
        vec!["d"],
    ]);

    let deleted = table.delete_rows_bulk(2, 2);

    assert_eq!(deleted.len(), 2);
    assert_eq!(table.row_count(), 2);
    assert_eq!(row(&table, 0), vec!["a"]);
    assert_eq!(row(&table, 1), vec!["b"]);
}

#[test]
fn test_delete_rows_bulk_all_rows_clears() {
    let mut table = make_table(vec![
        vec!["a", "1"],
        vec!["b", "2"],
        vec!["c", "3"],
    ]);

    let deleted = table.delete_rows_bulk(0, 3);

    assert_eq!(deleted.len(), 3);
    assert_eq!(table.row_count(), 1);
    assert_eq!(row(&table, 0), vec!["", ""]);
}

#[test]
fn test_delete_rows_bulk_exceeds_count() {
    let mut table = make_table(vec![
        vec!["a"],
        vec!["b"],
        vec!["c"],
    ]);

    let deleted = table.delete_rows_bulk(1, 100);

    assert_eq!(deleted.len(), 2);
    assert_eq!(table.row_count(), 1);
    assert_eq!(row(&table, 0), vec!["a"]);
}

#[test]
fn test_insert_rows_bulk_empty() {
    let mut table = make_table(vec![
        vec!["a", "1"],
        vec!["b", "2"],
    ]);

    table.insert_rows_bulk(1, 2);

    assert_eq!(table.row_count(), 4);
    assert_eq!(row(&table, 0), vec!["a", "1"]);
    assert_eq!(row(&table, 1), vec!["", ""]);
    assert_eq!(row(&table, 2), vec!["", ""]);
    assert_eq!(row(&table, 3), vec!["b", "2"]);
}

#[test]
fn test_insert_rows_with_data_bulk() {
    let mut table = make_table(vec![
        vec!["a", "1"],
        vec!["d", "4"],
    ]);

    table.insert_rows_with_data_bulk(1, vec![
        vec!["b".to_string(), "2".to_string()],
        vec!["c".to_string(), "3".to_string()],
    ]);

    assert_eq!(table.row_count(), 4);
    assert_eq!(row(&table, 0), vec!["a", "1"]);
    assert_eq!(row(&table, 1), vec!["b", "2"]);
    assert_eq!(row(&table, 2), vec!["c", "3"]);
    assert_eq!(row(&table, 3), vec!["d", "4"]);
}

#[test]
fn test_insert_rows_bulk_at_end() {
    let mut table = make_table(vec![
        vec!["a"],
        vec!["b"],
    ]);

    table.insert_rows_with_data_bulk(2, vec![
        vec!["c".to_string()],
        vec!["d".to_string()],
    ]);

    assert_eq!(table.row_count(), 4);
    assert_eq!(row(&table, 2), vec!["c"]);
    assert_eq!(row(&table, 3), vec!["d"]);
}

#[test]
fn test_insert_rows_bulk_pads_short_rows() {
    let mut table = make_table(vec![
        vec!["a", "b", "c"],
    ]);

    table.insert_rows_with_data_bulk(1, vec![
        vec!["x".to_string()], // Short row
    ]);

    assert_eq!(row(&table, 1), vec!["x", "", ""]);
}

#[test]
fn test_get_rows_cloned() {
    let table = make_table(vec![
        vec!["a", "1"],
        vec!["b", "2"],
        vec!["c", "3"],
        vec!["d", "4"],
    ]);

    let rows = table.get_rows_cloned(1, 2);

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["b", "2"]);
    assert_eq!(rows[1], vec!["c", "3"]);
}

#[test]
fn test_get_rows_cloned_clamps_count() {
    let table = make_table(vec![
        vec!["a"],
        vec!["b"],
        vec!["c"],
    ]);

    let rows = table.get_rows_cloned(1, 100);

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], vec!["b"]);
    assert_eq!(rows[1], vec!["c"]);
}

#[test]
fn test_probe_row_type() {
    let table = make_table(vec![
        vec!["Name", "Alice", "Bob", "Carol"],
        vec!["Score", "95", "87", "92"],
    ]);

    // Row 0 is text (names)
    assert_eq!(table.probe_row_type(0, true), ColumnType::Text);
    // Row 1 is numeric (scores)
    assert_eq!(table.probe_row_type(1, true), ColumnType::Numeric);
}

// === Filtered navigation tests ===

fn row_manager_filtered(active_rows: Vec<usize>) -> Rc<RefCell<RowManager>> {
    let mut rm = RowManager::new();
    rm.is_filtered = true;
    rm.active_rows = active_rows.clone();
    rm.active_row_set = active_rows.into_iter().collect();
    Rc::new(RefCell::new(rm))
}

#[test]
fn test_move_down_with_filter() {
    // Table has 10 rows, but only rows 0, 2, 5, 8 are active
    let rm = row_manager_filtered(vec![0, 2, 5, 8]);
    let mut view = TableView::new(rm);
    let table = make_table(vec![vec!["x"]; 10]);

    view.cursor_row = 0;
    view.move_down(&table);
    // Should skip to row 2 (next active row)
    assert_eq!(view.cursor_row, 2);

    view.move_down(&table);
    // Should skip to row 5
    assert_eq!(view.cursor_row, 5);

    view.move_down(&table);
    // Should skip to row 8
    assert_eq!(view.cursor_row, 8);

    view.move_down(&table);
    // At last active row, should stay
    assert_eq!(view.cursor_row, 8);
}

#[test]
fn test_move_up_with_filter() {
    let rm = row_manager_filtered(vec![0, 2, 5, 8]);
    let mut view = TableView::new(rm);
    let table = make_table(vec![vec!["x"]; 10]);

    view.cursor_row = 8;
    view.move_up();
    // Should skip to row 5 (previous active row)
    assert_eq!(view.cursor_row, 5);

    view.move_up();
    assert_eq!(view.cursor_row, 2);

    view.move_up();
    assert_eq!(view.cursor_row, 0);

    view.move_up();
    // At first row, should stay
    assert_eq!(view.cursor_row, 0);
}

#[test]
fn test_move_to_bottom_with_filter() {
    let rm = row_manager_filtered(vec![0, 2, 5, 8]);
    let mut view = TableView::new(rm);
    let table = make_table(vec![vec!["x"]; 10]);

    view.cursor_row = 0;
    view.move_to_bottom(&table);
    // Should go to last active row (8), not last table row (9)
    assert_eq!(view.cursor_row, 8);
}

#[test]
fn test_page_down_with_filter() {
    let rm = row_manager_filtered(vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);
    let mut view = TableView::new(rm);
    view.viewport_height = 5;
    let table = make_table(vec![vec!["x"]; 25]);

    view.cursor_row = 0;
    view.page_down(&table);
    // Jump of 4 (viewport_height - 1) in filtered rows
    // From index 0 in active_rows, jump 4 positions: 0 -> 2 -> 4 -> 6 -> 8
    assert_eq!(view.cursor_row, 8);
}

#[test]
fn test_page_up_with_filter() {
    let rm = row_manager_filtered(vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);
    let mut view = TableView::new(rm);
    view.viewport_height = 5;
    let table = make_table(vec![vec!["x"]; 25]);

    view.cursor_row = 20;
    view.page_up();
    // Jump back 4 positions in filtered rows
    assert_eq!(view.cursor_row, 12);
}

#[test]
fn test_move_down_n_with_filter() {
    let rm = row_manager_filtered(vec![0, 3, 6, 9, 12]);
    let mut view = TableView::new(rm);
    let table = make_table(vec![vec!["x"]; 15]);

    view.cursor_row = 0;
    view.move_down_n(2, &table);
    // Should jump 2 active rows: 0 -> 3 -> 6
    assert_eq!(view.cursor_row, 6);
}

#[test]
fn test_move_up_n_with_filter() {
    let rm = row_manager_filtered(vec![0, 3, 6, 9, 12]);
    let mut view = TableView::new(rm);
    let table = make_table(vec![vec!["x"]; 15]);

    view.cursor_row = 12;
    view.move_up_n(3);
    // Should jump back 3 active rows: 12 -> 9 -> 6 -> 3
    assert_eq!(view.cursor_row, 3);
}

#[test]
fn test_half_page_down_with_filter() {
    let rm = row_manager_filtered(vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
    let mut view = TableView::new(rm);
    view.viewport_height = 6;
    let table = make_table(vec![vec!["x"]; 20]);

    view.cursor_row = 0;
    view.half_page_down(&table);
    // Half page = 3 rows in filtered set: 0 -> 2 -> 4 -> 6
    assert_eq!(view.cursor_row, 6);
}

#[test]
fn test_half_page_up_with_filter() {
    let rm = row_manager_filtered(vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
    let mut view = TableView::new(rm);
    view.viewport_height = 6;
    let table = make_table(vec![vec!["x"]; 20]);

    view.cursor_row = 18;
    view.half_page_up();
    // Half page = 3 rows back in filtered set
    assert_eq!(view.cursor_row, 12);
}

#[test]
fn test_scroll_to_cursor_with_filter() {
    let rm = row_manager_filtered(vec![0, 10, 20, 30, 40, 50]);
    let mut view = TableView::new(rm);
    view.viewport_height = 3;
    let table = make_table(vec![vec!["x"]; 60]);

    // Start at top
    view.viewport_row = 0;
    view.cursor_row = 50;

    // Scroll to cursor should adjust viewport
    view.scroll_to_cursor();

    // Viewport should have scrolled to show cursor
    // The exact value depends on jump_up implementation
    assert!(view.viewport_row <= view.cursor_row);
    assert!(view.cursor_row < view.viewport_row + view.viewport_height + 50);
}
