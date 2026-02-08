use std::collections::HashSet;

use crate::table::table::Table;
use crate::numeric::predicate::Predicate;
use crate::util::{letters_from_col, ColumnType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterType {
    Default,
    PredicateFilter(Predicate)
}

/// Snapshot of filter state for undo/redo
#[derive(Debug, Clone, PartialEq)]
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

    pub fn should_scroll(&self, cursor_row: usize, viewport_row: usize, viewport_height: usize) -> bool {
        if self.is_filtered {
            let cursor_idx = self.active_rows.partition_point(|&val| val < cursor_row);
            let viewport_idx = self.active_rows.partition_point(|&val| val < viewport_row);

            cursor_idx >= viewport_idx + viewport_height
        } else {
            cursor_row >= viewport_row + viewport_height
        }
    }

    pub fn predicate_filter(&mut self, table: &Table, col: usize, predicate: Predicate, col_type: ColumnType, keep_header: bool) {
        let idxs: Box<dyn Iterator<Item = usize>> = if self.is_filtered {
            Box::new(self.active_rows.iter().map(|&i| i))
        } else {
            Box::new(0usize..table.row_count())
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
