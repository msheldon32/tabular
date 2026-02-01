use std::collections::HashSet;

use crate::table::Table;
use crate::predicate::Predicate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterType {
    Default,
    Fibonacci,
    PredicateFilter(Predicate)
}

#[derive(Debug, Clone)]
pub struct RowManager {
    pub is_filtered: bool,
    pub active_rows: Vec<usize>,
    pub active_row_set: HashSet<usize>
}

impl RowManager {
    pub fn new() -> Self {
        Self {
            is_filtered: false,
            active_rows: Vec::new(),
            active_row_set: HashSet::new()
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
    }

    pub fn remove_filter(&mut self) {
        self.active_rows = Vec::new();
        self.active_row_set = HashSet::new();
        self.is_filtered = false;
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
}
