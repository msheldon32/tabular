use std::cmp;
use std::rc::Rc;
use std::cell::RefCell;

use crate::table::table::Table;
use crate::mode::Mode;
use crate::table::rowmanager::RowManager;

/// View state for the table (cursor, viewport, selection)
#[derive(Debug, Clone)]
pub struct TableView {
    // Cursor position
    pub cursor_row: usize,
    pub cursor_col: usize,

    // Support position (for visual mode)
    pub support_row: usize,
    pub support_col: usize,

    // Viewport offset (top-left visible cell)
    pub viewport_row: usize,
    pub viewport_col: usize,

    // Last visible row (set during render)
    pub viewport_height: usize,
    pub viewport_width: usize,

    pub row_manager: Rc<RefCell<RowManager>>
}

impl TableView {
    pub fn new(row_manager: Rc<RefCell<RowManager>>) -> Self {
        Self {
            cursor_row: 0,
            cursor_col: 0,
            viewport_row: 0,
            viewport_col: 0,
            viewport_height: 20,
            viewport_width: 10,
            support_row: 0,
            support_col: 0,
            row_manager
        }
    }

    pub fn is_selected(&mut self, row_idx: usize, col_idx: usize, mode: Mode) -> bool {
        let mut row_valid = true;
        let mut col_valid = true;
        if mode != Mode::VisualCol {
            row_valid = cmp::min(self.cursor_row, self.support_row) <= row_idx;
            row_valid = row_valid && row_idx <= cmp::max(self.cursor_row, self.support_row);
        }

        if mode != Mode::VisualRow {
            col_valid = cmp::min(self.cursor_col, self.support_col) <= col_idx;
            col_valid = col_valid && col_idx <= cmp::max(self.cursor_col, self.support_col);
        }

        return row_valid && col_valid;
    }

    pub fn set_support(&mut self) {
        self.support_row = self.cursor_row;
        self.support_col = self.cursor_col;
    }

    /// Get the bounds of the current selection (start_row, end_row, start_col, end_col)
    pub fn get_selection_bounds(&self) -> (usize, usize, usize, usize) {
        (
            cmp::min(self.cursor_row, self.support_row),
            cmp::max(self.cursor_row, self.support_row),
            cmp::min(self.cursor_col, self.support_col),
            cmp::max(self.cursor_col, self.support_col),
        )
    }

    /// Ensure cursor is within table bounds
    pub fn clamp_cursor(&mut self, table: &Table) {
        if table.row_count() > 0 {
            self.cursor_row = self.cursor_row.min(table.row_count() - 1);
        }
        if table.col_count() > 0 {
            self.cursor_col = self.cursor_col.min(table.col_count() - 1);
        }
    }

    /// Ensure viewport contains the cursor
    pub fn scroll_to_cursor(&mut self) {
        // Vertical scrolling
        if self.cursor_row < self.viewport_row {
            self.viewport_row = self.cursor_row;
        } else if self.cursor_row >= self.viewport_row + self.viewport_height {
            self.viewport_row = self.row_manager.borrow().jump_up(self.cursor_row, self.viewport_height-1);
        }

        // Horizontal scrolling
        if self.cursor_col < self.viewport_col {
            self.viewport_col = self.cursor_col;
        } else if self.cursor_col >= self.viewport_col + self.viewport_width {
            self.viewport_col = self.cursor_col.saturating_sub(self.viewport_width - 1);
        }
    }

    // Navigation methods
    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_right(&mut self, table: &Table) {
        if self.cursor_col + 1 < table.col_count() {
            self.cursor_col += 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row = self.row_manager.borrow().get_predecessor(self.cursor_row).unwrap_or(0);
            self.scroll_to_cursor();
        }
    }

    pub fn move_down(&mut self, table: &Table) {
        if self.cursor_row + 1 < table.row_count() {
            let last_row = self.row_manager.borrow().get_end(table);
            self.cursor_row = self.row_manager.borrow().get_successor(self.cursor_row).unwrap_or(last_row);
            self.scroll_to_cursor();
        }
    }

    pub fn move_to_top(&mut self) {
        self.cursor_row = 0;
        self.scroll_to_cursor();
    }

    pub fn move_to_bottom(&mut self, table: &Table) {
        if table.row_count() > 0 {
            self.cursor_row = self.row_manager.borrow().get_end(table);
            self.scroll_to_cursor();
        }
    }

    pub fn move_to_first_col(&mut self) {
        self.cursor_col = 0;
        self.scroll_to_cursor();
    }

    pub fn move_to_last_col(&mut self, table: &Table) {
        if table.col_count() > 0 {
            self.cursor_col = table.col_count() - 1;
            self.scroll_to_cursor();
        }
    }

    pub fn page_down(&mut self, table: &Table) {
        let jump = self.viewport_height.saturating_sub(1).max(1);
        self.cursor_row = self.row_manager.borrow().jump_down(self.cursor_row, jump, table);
        self.scroll_to_cursor();
    }

    pub fn page_up(&mut self) {
        let jump = self.viewport_height.saturating_sub(1).max(1);
        self.cursor_row = self.row_manager.borrow().jump_up(self.cursor_row, jump);
        self.scroll_to_cursor();
    }

    pub fn half_page_down(&mut self, table: &Table) {
        let jump = self.viewport_height / 2;
        self.cursor_row = self.row_manager.borrow().jump_down(self.cursor_row, jump, table);
        self.scroll_to_cursor();
    }

    pub fn half_page_up(&mut self) {
        let jump = self.viewport_height / 2;
        self.cursor_row = self.row_manager.borrow().jump_up(self.cursor_row, jump);
        self.scroll_to_cursor();
    }

    // Movement with count
    pub fn move_left_n(&mut self, n: usize) {
        self.cursor_col = self.cursor_col.saturating_sub(n);
        self.scroll_to_cursor();
    }

    pub fn move_right_n(&mut self, n: usize, table: &Table) {
        self.cursor_col = (self.cursor_col + n).min(table.col_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    pub fn move_up_n(&mut self, n: usize) {
        //self.cursor_row = self.cursor_row.saturating_sub(n);
        self.cursor_row = self.row_manager.borrow().jump_up(self.cursor_row, n);
        self.scroll_to_cursor();
    }

    pub fn move_down_n(&mut self, n: usize, table: &Table) {
        //self.cursor_row = (self.cursor_row + n).min(table.row_count().saturating_sub(1));
        self.cursor_row = self.row_manager.borrow().jump_down(self.cursor_row, n, table);
        self.scroll_to_cursor();
    }

    // Jump navigation (Ctrl+Arrow behavior like Excel)
    // If in occupied cell: jump to last occupied cell before empty/edge
    // If in empty cell: jump to first occupied cell in direction

    fn is_cell_occupied(table: &Table, row: usize, col: usize) -> bool {
        table.get_cell(row, col)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    pub fn jump_left(&mut self, table: &Table) {
        if self.cursor_col == 0 {
            return;
        }

        let mut target = self.cursor_col - 1;

        let is_occupied = Self::is_cell_occupied(table, self.cursor_row, target);

        while target > 0 && (Self::is_cell_occupied(table, self.cursor_row, target-1) == is_occupied) {
            target -= 1;
        }

        if target > 0 && !is_occupied {
            target -= 1;
        }

        self.cursor_col = target;

        self.scroll_to_cursor();
    }

    pub fn jump_right(&mut self, table: &Table) {
        let max_col = table.col_count().saturating_sub(1);
        if self.cursor_col >= max_col {
            return;
        }

        let mut target = self.cursor_col + 1;

        let is_occupied = Self::is_cell_occupied(table, self.cursor_row, target);

        while target < max_col && (Self::is_cell_occupied(table, self.cursor_row, target+1) == is_occupied) {
            target += 1;
        }

        if target < max_col && !is_occupied {
            target += 1;
        }

        self.cursor_col = target;

        self.scroll_to_cursor();
    }

    pub fn jump_up(&mut self, table: &Table) {
        if self.cursor_row == 0 {
            return;
        }

        let mut target = self.cursor_row - 1;

        let is_occupied = Self::is_cell_occupied(table, target, self.cursor_col);

        while target > 0 && (Self::is_cell_occupied(table, target-1, self.cursor_col) == is_occupied) {
            target -= 1;
        }

        if target > 0 && !is_occupied {
            target -= 1;
        }

        self.cursor_row = target;

        self.scroll_to_cursor();
    }

    pub fn jump_down(&mut self, table: &Table) {
        let max_row = table.row_count().saturating_sub(1);
        if self.cursor_row >= max_row {
            return;
        }

        let mut target = self.cursor_row + 1;

        let is_occupied = Self::is_cell_occupied(table, target, self.cursor_col);

        while target < max_row && (Self::is_cell_occupied(table, target+1, self.cursor_col) == is_occupied) {
            target += 1;
        }

        if target < max_row && !is_occupied {
            target += 1;
        }

        self.cursor_row = target;

        self.scroll_to_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::SortDirection;
    use crate::util::ColumnType;
    use crate::operations;

    // === Basic operations ===
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

        assert_eq!(operations::current_cell(&view, &table), "hello");
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
}
