use std::collections::HashSet;

use crate::table::Table;
use crate::predicate::Predicate;
use crate::util::{letters_from_col, ColumnType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterType {
    Default,
    Fibonacci,
    PredicateFilter(Predicate)
}

/// Snapshot of filter state for undo/redo
#[derive(Debug, Clone)]
pub struct FilterState {
    pub is_filtered: bool,
    pub active_rows: Vec<usize>,
    pub filter_string: String,
}

#[derive(Debug, Clone)]
pub struct RowManager {
    pub is_filtered: bool,
    pub active_rows: Vec<usize>,
    pub active_row_set: HashSet<usize>,
    pub filter_string: String
}

impl RowManager {
    pub fn new() -> Self {
        Self {
            is_filtered: false,
            active_rows: Vec::new(),
            active_row_set: HashSet::new(),
            filter_string: String::new()
        }
    }

    pub fn is_row_live(&self, row: usize) -> bool {
        if self.is_filtered {
            self.active_row_set.contains(&row)
        } else {
            true
        }
    }

    pub fn row_closure(self) -> impl Fn(usize) -> bool {
        move |i| self.is_row_live(i)
    }

    pub fn row_closure_wf(self) -> impl Fn(&usize) -> bool {
        move |&i| self.is_row_live(i)
    }

    pub fn get_successor(&self, row: usize) -> Option<usize> {
        // note that this does *not* check the table size
        if self.is_filtered {
            self.active_rows.get(self.active_rows.partition_point(|&val| val <= row)).copied()
        } else {
            Some(row + 1)
        }
    }

    pub fn get_predecessor(&self, row: usize) -> Option<usize> {
        // note that this does *not* check the table size
        if self.is_filtered {
            self.active_rows.partition_point(|&val| val < row).checked_sub(1).and_then(|i| self.active_rows.get(i)).copied()
        } else {
            row.checked_sub(1)
        }
    }

    pub fn get_end(&self, table: &Table) -> usize {
        if self.is_filtered {
            self.active_rows.last().copied().unwrap_or(0usize)
        } else {
            table.row_count()-1
        }
    }

    pub fn jump_down(&self, start: usize, jump: usize, table: &Table) -> usize {
        if self.is_filtered {
            self.active_rows.get(self.active_rows.partition_point(|&val| val <= start) + jump - 1).unwrap_or(&self.get_end(table)).clone()
        } else {
            (start+jump).min(table.row_count().saturating_sub(1))
        }
    }

    pub fn jump_up(&self, start: usize, jump: usize) -> usize {
        if jump == 0 {
            start
        } else if self.is_filtered {
            self.active_rows.get(self.active_rows.partition_point(|&val| val < start).saturating_sub(jump)).unwrap_or(&0usize).clone()
        } else {
            start.saturating_sub(jump)
        }
    }

    pub fn fibonacci_filter(&mut self, table: &Table) {
        if !self.is_filtered {
            self.active_rows = vec![0,1,1,2,3,5,8,13,21,35,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78];
        } else {
            self.active_rows = vec![0,1,1,2,3,5,8,13,21,35,56];
        }
        self.active_row_set = HashSet::from_iter(self.active_rows.iter().cloned());
        self.is_filtered = true;
        self.filter_string = String::from("Filtered (fibonacci)");
    }

    pub fn predicate_filter(&mut self, table: &Table, col: usize, predicate: Predicate, col_type: ColumnType, keep_header: bool) {
        let idxs: Box<dyn Iterator<Item = usize>> = if self.is_filtered {
            Box::new(self.active_rows.iter().map(|&i| i))
        } else {
            Box::new((0usize..table.row_count()))
        };

        self.active_rows = idxs.filter(|&i| predicate.evaluate(table.get_cell(i, col).unwrap(), col_type)).collect();

        if keep_header && self.active_rows.first() != Some(&0usize) {
            self.active_rows.insert(0, 0usize);
        }

        self.active_row_set = HashSet::from_iter(self.active_rows.iter().cloned());
        self.is_filtered = true;
        let col_letter = letters_from_col(col);
        self.filter_string = format!("Filtered ({} {})", col_letter, predicate.to_string());
    }

    pub fn remove_filter(&mut self) {
        self.active_rows = Vec::new();
        self.active_row_set = HashSet::new();
        self.is_filtered = false;
        self.filter_string = String::new();
    }

    /// Capture current filter state for undo/redo
    pub fn snapshot(&self) -> FilterState {
        FilterState {
            is_filtered: self.is_filtered,
            active_rows: self.active_rows.clone(),
            filter_string: self.filter_string.clone(),
        }
    }

    /// Restore filter state from a snapshot
    pub fn restore(&mut self, state: FilterState) {
        self.is_filtered = state.is_filtered;
        self.active_rows = state.active_rows;
        self.active_row_set = self.active_rows.iter().cloned().collect();
        self.filter_string = state.filter_string;
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
            op: crate::predicate::Op::Gt,
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
            op: crate::predicate::Op::Eq,
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
            op: crate::predicate::Op::Ge,
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
            op: crate::predicate::Op::Eq,
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
            op: crate::predicate::Op::Gt,
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
            op: crate::predicate::Op::Gt,
            val: "80".to_string(),
        };
        rm.predicate_filter(&table, 1, pred1, crate::util::ColumnType::Numeric, true);
        assert_eq!(rm.active_rows, vec![0, 1, 2, 3, 5]); // header + Alice, Bob, Carol, Eve

        // Second filter: score < 95 (should chain with first filter)
        let pred2 = Predicate::Comparator {
            op: crate::predicate::Op::Lt,
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
            op: crate::predicate::Op::Le,
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
            op: crate::predicate::Op::Ge,
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
            op: crate::predicate::Op::Ge,
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
            op: crate::predicate::Op::Gt,
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
            op: crate::predicate::Op::Gt,
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
            op: crate::predicate::Op::Gt,
            val: "100".to_string(),
        };

        rm.predicate_filter(&table, 1, pred, crate::util::ColumnType::Numeric, true);

        // No data rows match, but header should be preserved
        assert!(rm.is_filtered);
        assert_eq!(rm.active_rows, vec![0]);
    }
}
