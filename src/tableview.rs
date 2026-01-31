use std::cmp;

use crate::table::{Table, SortType, SortDirection};
use crate::util::translate_references;
use crate::mode::Mode;

/// View state for the table (cursor, viewport, cached widths)
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

    // Visible area size (set during render)
    pub visible_rows: usize,
    pub visible_cols: usize,

    // Cached column widths
    pub col_widths: Vec<usize>,
}

impl TableView {
    pub fn new() -> Self {
        Self {
            cursor_row: 0,
            cursor_col: 0,
            viewport_row: 0,
            viewport_col: 0,
            visible_rows: 20,
            visible_cols: 10,
            support_row: 0,
            support_col: 0,
            col_widths: Vec::new(),
        }
    }

    /// Update cached column widths based on table content
    /// Now delegates to Table's cached widths
    pub fn update_col_widths(&mut self, table: &Table) {
        self.col_widths = table.col_widths_cached().to_vec();
    }

    /// Sync column widths from a mutable table (forces recompute if dirty)
    pub fn sync_col_widths(&mut self, table: &mut Table) {
        self.col_widths = table.col_widths().to_vec();
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

    pub fn expand_column(&mut self, length: usize) {
        self.col_widths[self.cursor_col] = cmp::max(self.col_widths[self.cursor_col], length);
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
        } else if self.cursor_row >= self.viewport_row + self.visible_rows {
            self.viewport_row = self.cursor_row.saturating_sub(self.visible_rows - 1);
        }

        // Horizontal scrolling
        if self.cursor_col < self.viewport_col {
            self.viewport_col = self.cursor_col;
        } else if self.cursor_col >= self.viewport_col + self.visible_cols {
            self.viewport_col = self.cursor_col.saturating_sub(self.visible_cols - 1);
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
            self.cursor_row -= 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_down(&mut self, table: &Table) {
        if self.cursor_row + 1 < table.row_count() {
            self.cursor_row += 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_to_top(&mut self) {
        self.cursor_row = 0;
        self.scroll_to_cursor();
    }

    pub fn move_to_bottom(&mut self, table: &Table) {
        if table.row_count() > 0 {
            self.cursor_row = table.row_count() - 1;
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
        let jump = self.visible_rows.saturating_sub(1).max(1);
        self.cursor_row = (self.cursor_row + jump).min(table.row_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    pub fn page_up(&mut self) {
        let jump = self.visible_rows.saturating_sub(1).max(1);
        self.cursor_row = self.cursor_row.saturating_sub(jump);
        self.scroll_to_cursor();
    }

    pub fn half_page_down(&mut self, table: &Table) {
        let jump = self.visible_rows / 2;
        self.cursor_row = (self.cursor_row + jump).min(table.row_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    pub fn half_page_up(&mut self) {
        let jump = self.visible_rows / 2;
        self.cursor_row = self.cursor_row.saturating_sub(jump);
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
        self.cursor_row = self.cursor_row.saturating_sub(n);
        self.scroll_to_cursor();
    }

    pub fn move_down_n(&mut self, n: usize, table: &Table) {
        self.cursor_row = (self.cursor_row + n).min(table.row_count().saturating_sub(1));
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

    pub fn goto_row(&mut self, row: usize, table: &Table) {
        self.cursor_row = row.min(table.row_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    /// Get current cell content
    pub fn current_cell<'a>(&self, table: &'a Table) -> &'a String {
        table.get_cell(self.cursor_row, self.cursor_col)
            .expect("Cursor should be within bounds")
    }

    /// Get mutable reference to current cell
    pub fn current_cell_mut<'a>(&self, table: &'a mut Table) -> &'a mut String {
        table.get_row_mut(self.cursor_row)
            .and_then(|r| r.get_mut(self.cursor_col))
            .expect("Cursor should be within bounds")
    }

    // Row operations that update cursor
    pub fn insert_row_below(&mut self, table: &mut Table) {
        table.insert_row_at(self.cursor_row + 1);
        self.cursor_row += 1;
        self.scroll_to_cursor();
    }

    pub fn insert_row_above(&mut self, table: &mut Table) {
        table.insert_row_at(self.cursor_row);
        self.scroll_to_cursor();
    }

    pub fn delete_row(&mut self, table: &mut Table) -> Option<Vec<String>> {
        let row = table.delete_row_at(self.cursor_row);
        self.clamp_cursor(table);
        self.scroll_to_cursor();
        row
    }

    /// Delete multiple selected rows (for VisualRow mode), returns deleted rows
    pub fn delete_rows_bulk(&mut self, table: &mut Table) -> Vec<Vec<String>> {
        let (start_row, end_row, _, _) = self.get_selection_bounds();
        let count = end_row - start_row + 1;
        let deleted = table.delete_rows_bulk(start_row, count);
        self.cursor_row = start_row;
        self.support_row = start_row;
        self.clamp_cursor(table);
        self.scroll_to_cursor();
        deleted
    }

    pub fn yank_row(&self, table: &Table) -> Option<Vec<String>> {
        table.get_row_cloned(self.cursor_row)
    }

    /// Yank multiple selected rows (for VisualRow mode)
    pub fn yank_rows_bulk(&self, table: &Table) -> Vec<Vec<String>> {
        let (start_row, end_row, _, _) = self.get_selection_bounds();
        let count = end_row - start_row + 1;
        table.get_rows_cloned(start_row, count)
    }

    pub fn paste_row(&mut self, table: &mut Table, row: Vec<String>) {
        table.fill_row_with_data(self.cursor_row, row);
    }

    /// Paste multiple rows starting at cursor, overwriting existing rows
    pub fn paste_rows_bulk(&mut self, table: &mut Table, rows: Vec<Vec<String>>) {
        table.fill_rows_with_data_bulk(self.cursor_row, rows);
    }

    /// Insert multiple rows below current selection with data (e.g., after paste)
    pub fn insert_rows_below_bulk(&mut self, table: &mut Table, rows: Vec<Vec<String>>) {
        let count = rows.len();
        let insert_at = self.cursor_row + 1;
        table.insert_rows_with_data_bulk(insert_at, rows);
        self.cursor_row = insert_at;
        self.support_row = insert_at + count - 1;
        self.scroll_to_cursor();
    }

    /// Insert multiple empty rows below cursor
    pub fn insert_rows_below_empty(&mut self, table: &mut Table, count: usize) {
        let insert_at = self.cursor_row + 1;
        table.insert_rows_bulk(insert_at, count);
        self.cursor_row = insert_at;
        self.support_row = insert_at + count - 1;
        self.scroll_to_cursor();
    }

    /// Insert multiple rows above current selection with data
    pub fn insert_rows_above_bulk(&mut self, table: &mut Table, rows: Vec<Vec<String>>) {
        let count = rows.len();
        table.insert_rows_with_data_bulk(self.cursor_row, rows);
        self.support_row = self.cursor_row + count - 1;
        self.scroll_to_cursor();
    }

    // Column operations that update cursor
    pub fn insert_col_after(&mut self, table: &mut Table) {
        table.insert_col_at(self.cursor_col + 1);
        self.update_col_widths(table);
    }

    pub fn delete_col(&mut self, table: &mut Table) -> Option<Vec<String>> {
        let col = table.delete_col_at(self.cursor_col);
        self.clamp_cursor(table);
        self.update_col_widths(table);
        self.scroll_to_cursor();
        col
    }

    pub fn yank_col(&self, table: &Table) -> Option<Vec<String>> {
        table.get_col_cloned(self.cursor_col)
    }

    pub fn paste_col(&mut self, table: &mut Table, col: Vec<String>) {
        table.fill_col_with_data(self.cursor_col, col);
        self.update_col_widths(table);
    }

    pub fn yank_span(&self, table: &Table) -> Option<Vec<Vec<String>>> {
        let (start_row, end_row, start_col, end_col) = self.get_selection_bounds();
        table.get_span(start_row, end_row, start_col, end_col)
    }

    pub fn paste_span(&mut self, table: &mut Table, span: Vec<Vec<String>>) {
        table.fill_span_with_data(self.cursor_row, self.cursor_col, span);
    }

    pub fn clear_span(&mut self, table: &mut Table) {
        let (start_row, end_row, start_col, end_col) = self.get_selection_bounds();
        for row_idx in start_row..=end_row {
            for col_idx in start_col..=end_col {
                table.set_cell(row_idx, col_idx, String::new());
            }
        }
    }

    pub fn clear_row_span(&mut self, table: &mut Table) {
        let (start_row, end_row, _, _) = self.get_selection_bounds();
        let col_count = table.col_count();
        for row_idx in start_row..=end_row {
            for col_idx in 0..col_count {
                table.set_cell(row_idx, col_idx, String::new());
            }
        }
    }

    pub fn clear_col_span(&mut self, table: &mut Table) {
        let (_, _, start_col, end_col) = self.get_selection_bounds();
        let row_count = table.row_count();
        for row_idx in 0..row_count {
            for col_idx in start_col..=end_col {
                table.set_cell(row_idx, col_idx, String::new());
            }
        }
    }

    pub fn drag_down(&mut self, table: &mut Table, whole_row: bool) {
        let (start_row, end_row, sel_start_col, sel_end_col) = self.get_selection_bounds();
        let (start_col, end_col) = if whole_row {
            (0, table.col_count() - 1)
        } else {
            (sel_start_col, sel_end_col)
        };

        for row_idx in start_row+1..=end_row {
            for col_idx in start_col..=end_col {
                let source = table.get_cell(start_row, col_idx).cloned().unwrap_or_default();
                let new_val = translate_references(&source, (row_idx - start_row) as isize, 0isize);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }

    pub fn drag_up(&mut self, table: &mut Table, whole_row: bool) {
        let (start_row, end_row, sel_start_col, sel_end_col) = self.get_selection_bounds();
        let (start_col, end_col) = if whole_row {
            (0, table.col_count() - 1)
        } else {
            (sel_start_col, sel_end_col)
        };

        for row_idx in start_row..end_row {
            let offset = row_idx as isize - end_row as isize;
            for col_idx in start_col..=end_col {
                let source = table.get_cell(end_row, col_idx).cloned().unwrap_or_default();
                let new_val = translate_references(&source, offset, 0isize);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }

    pub fn drag_right(&mut self, table: &mut Table, whole_col: bool) {
        let (sel_start_row, sel_end_row, start_col, end_col) = self.get_selection_bounds();
        let (start_row, end_row) = if whole_col {
            (0, table.row_count() - 1)
        } else {
            (sel_start_row, sel_end_row)
        };

        for row_idx in start_row..=end_row {
            for col_idx in start_col+1..=end_col {
                let source = table.get_cell(row_idx, start_col).cloned().unwrap_or_default();
                let new_val = translate_references(&source, 0isize, (col_idx - start_col) as isize);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }

    pub fn drag_left(&mut self, table: &mut Table, whole_col: bool) {
        let (sel_start_row, sel_end_row, start_col, end_col) = self.get_selection_bounds();
        let (start_row, end_row) = if whole_col {
            (0, table.row_count() - 1)
        } else {
            (sel_start_row, sel_end_row)
        };

        for row_idx in start_row..=end_row {
            for col_idx in start_col..end_col {
                let offset = col_idx as isize - end_col as isize;
                let source = table.get_cell(row_idx, end_col).cloned().unwrap_or_default();
                let new_val = translate_references(&source, 0isize, offset);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }
}

impl Default for TableView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;


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

    // === TableView unit tests ===
    #[test]
    fn test_tableview_new() {
        let view = TableView::new();
        assert_eq!(view.cursor_row, 0);
        assert_eq!(view.cursor_col, 0);
        assert_eq!(view.viewport_row, 0);
        assert_eq!(view.viewport_col, 0);
    }

    #[test]
    fn test_tableview_navigation() {
        let mut view = TableView::new();
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
        let mut view = TableView::new();
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
        let mut view = TableView::new();
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
        let mut view = TableView::new();
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
        let view = TableView::new();
        let table = make_table(vec![
            vec!["hello", "world"],
        ]);

        assert_eq!(view.current_cell(&table), "hello");
    }

    #[test]
    fn test_tableview_update_col_widths() {
        let mut view = TableView::new();
        let table = make_table(vec![
            vec!["a", "longer", "x"],
            vec!["bb", "y", "shortest"],
        ]);

        view.update_col_widths(&table);

        assert_eq!(view.col_widths[0], 3); // min width is 3
        assert_eq!(view.col_widths[1], 6); // "longer"
        assert_eq!(view.col_widths[2], 8); // "shortest"
    }

    #[test]
    fn test_tableview_set_support() {
        let mut view = TableView::new();
        view.cursor_row = 5;
        view.cursor_col = 3;

        view.set_support();

        assert_eq!(view.support_row, 5);
        assert_eq!(view.support_col, 3);
    }

    #[test]
    fn test_tableview_yank_row() {
        let view = TableView::new();
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);

        let row = view.yank_row(&table).unwrap();
        assert_eq!(row, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_tableview_yank_col() {
        let view = TableView::new();
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
            vec!["e", "f"],
        ]);

        let col = view.yank_col(&table).unwrap();
        assert_eq!(col, vec!["a".to_string(), "c".to_string(), "e".to_string()]);
    }

    #[test]
    fn test_tableview_yank_span() {
        let mut view = TableView::new();
        view.cursor_row = 0;
        view.cursor_col = 0;
        view.support_row = 1;
        view.support_col = 1;

        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        let span = view.yank_span(&table).unwrap();
        assert_eq!(span, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["d".to_string(), "e".to_string()],
        ]);
    }

    #[test]
    fn test_tableview_is_selected_visual() {
        let mut view = TableView::new();
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
        let mut view = TableView::new();
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
        let mut view = TableView::new();
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
        let mut view = TableView::new();
        view.visible_rows = 10;

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

    #[test]
    fn test_tableview_expand_column() {
        let mut view = TableView::new();
        view.col_widths = vec![5, 5, 5];
        view.cursor_col = 1;

        view.expand_column(10);
        assert_eq!(view.col_widths[1], 10);

        // Shouldn't shrink
        view.expand_column(3);
        assert_eq!(view.col_widths[1], 10);
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
        assert_eq!(table.probe_column_type(0, true), SortType::Text);
        // Column 1 is numeric (scores)
        assert_eq!(table.probe_column_type(1, true), SortType::Numeric);
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
        assert_eq!(table.probe_column_type(0, true), SortType::Numeric);
        // Column 1 is mixed but majority numeric
        assert_eq!(table.probe_column_type(1, true), SortType::Numeric);
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
        assert_eq!(table.probe_column_type(0, true), SortType::Numeric);
    }

    #[test]
    fn test_probe_column_type_all_text() {
        let table = make_table(vec![
            vec!["Names"],
            vec!["Alice"],
            vec!["Bob"],
            vec!["Carol"],
        ]);

        assert_eq!(table.probe_column_type(0, true), SortType::Text);
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
    fn test_reorder_rows() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
        ]);

        let old = table.reorder_rows(&[2, 0, 1]);

        assert_eq!(row(&table, 0), vec!["c", "3"]);
        assert_eq!(row(&table, 1), vec!["a", "1"]);
        assert_eq!(row(&table, 2), vec!["b", "2"]);

        // Old data should be preserved for undo
        assert_eq!(old[0], vec!["a", "1"]);
        assert_eq!(old[1], vec!["b", "2"]);
        assert_eq!(old[2], vec!["c", "3"]);
    }

    #[test]
    fn test_reorder_cols() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["1", "2", "3"],
        ]);

        let old = table.reorder_cols(&[2, 0, 1]);

        assert_eq!(row(&table, 0), vec!["c", "a", "b"]);
        assert_eq!(row(&table, 1), vec!["3", "1", "2"]);

        // Old data should be preserved
        assert_eq!(old[0], vec!["a", "b", "c"]);
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

    // === Large-scale sorting tests ===

    /// Helper to create a large table with random-ish numeric values for sorting tests
    fn make_large_sortable_table(num_rows: usize) -> Table {
        // Create rows with values that will sort in a known order
        // Use a simple formula: value = (num_rows - i) to create reverse order
        let rows: Vec<Vec<String>> = (0..num_rows)
            .map(|i| vec![
                format!("{}", num_rows - i),  // Numeric col (reverse order)
                format!("name{:05}", i),      // Text col (forward order)
            ])
            .collect();
        Table::new(rows)
    }

    #[test]
    fn test_sort_rows_bulk_large_ascending() {
        // 3000 rows spanning 3 chunks
        let mut table = make_large_sortable_table(3000);
        assert_eq!(table.row_count(), 3000);

        // First column has values 3000, 2999, 2998, ..., 1
        // After ascending sort, should be 1, 2, 3, ..., 3000
        assert_eq!(table.get_cell(0, 0), Some(&"3000".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"1".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, false);
        assert!(old_data.is_some(), "Sort should have changed the order");

        // Verify sorted order
        for i in 0..3000 {
            let expected = format!("{}", i + 1);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }

        // Verify row count unchanged
        assert_eq!(table.row_count(), 3000);
    }

    #[test]
    fn test_sort_rows_bulk_large_descending() {
        // Create table with ascending values
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("{}", i + 1)])
            .collect();
        let mut table = Table::new(rows);

        // First column has values 1, 2, 3, ..., 3000
        assert_eq!(table.get_cell(0, 0), Some(&"1".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"3000".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Descending, false);
        assert!(old_data.is_some());

        // After descending sort: 3000, 2999, ..., 1
        for i in 0..3000 {
            let expected = format!("{}", 3000 - i);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }
    }

    #[test]
    fn test_sort_rows_bulk_large_with_header() {
        // Create table with header row
        let mut rows: Vec<Vec<String>> = vec![vec!["Value".to_string()]];
        rows.extend((0..2999).map(|i| vec![format!("{}", 2999 - i)]));
        let mut table = Table::new(rows);

        assert_eq!(table.row_count(), 3000);
        assert_eq!(table.get_cell(0, 0), Some(&"Value".to_string()));
        assert_eq!(table.get_cell(1, 0), Some(&"2999".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, true);
        assert!(old_data.is_some());

        // Header should remain at row 0
        assert_eq!(table.get_cell(0, 0), Some(&"Value".to_string()));

        // Data rows should be sorted: 1, 2, 3, ..., 2999
        for i in 1..3000 {
            let expected = format!("{}", i);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }
    }

    #[test]
    fn test_sort_rows_bulk_large_text() {
        // Create table with text values that span chunks
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("item{:05}", 2999 - i)])
            .collect();
        let mut table = Table::new(rows);

        // Should be: item02999, item02998, ..., item00000
        assert_eq!(table.get_cell(0, 0), Some(&"item02999".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"item00000".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, false);
        assert!(old_data.is_some());

        // After sort: item00000, item00001, ..., item02999
        for i in 0..3000 {
            let expected = format!("item{:05}", i);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }
    }

    #[test]
    fn test_sort_rows_bulk_already_sorted() {
        // Create already sorted table
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("{}", i + 1)])
            .collect();
        let mut table = Table::new(rows);

        // Should return None since already sorted
        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, false);
        assert!(old_data.is_none(), "Already sorted table should return None");
    }

    #[test]
    fn test_sort_preserves_row_integrity() {
        // Create table with multiple columns to verify entire rows move together
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![
                format!("{}", 3000 - i),           // Sort key (reverse)
                format!("row{}", i),               // Row identifier
                format!("data{}", i * 2),          // Associated data
            ])
            .collect();
        let mut table = Table::new(rows);

        table.sort_rows_by_column(0, SortDirection::Ascending, false);

        // Verify row integrity - each row's columns should still correspond
        for i in 0..3000 {
            let sort_key = table.get_cell(i, 0).unwrap();
            let row_id = table.get_cell(i, 1).unwrap();
            let data = table.get_cell(i, 2).unwrap();

            // sort_key should be i+1 (after ascending sort)
            assert_eq!(sort_key, &format!("{}", i + 1));

            // Original row index was (3000 - (i+1)) = 2999 - i
            let orig_idx = 2999 - i;
            assert_eq!(row_id, &format!("row{}", orig_idx),
                "Row {} should have id row{}", i, orig_idx);
            assert_eq!(data, &format!("data{}", orig_idx * 2),
                "Row {} should have data{}", i, orig_idx * 2);
        }
    }

    #[test]
    fn test_reorder_rows_bulk_returns_old_data() {
        let mut table = make_large_sortable_table(3000);

        // Get original first and last values
        let orig_first = table.get_cell(0, 0).unwrap().clone();
        let orig_last = table.get_cell(2999, 0).unwrap().clone();

        // Create reverse order
        let new_order: Vec<usize> = (0..3000).rev().collect();
        let old_data = table.reorder_rows_bulk(&new_order);

        // Verify old_data contains original values
        assert_eq!(old_data.len(), 3000);
        assert_eq!(old_data[0][0], orig_first);
        assert_eq!(old_data[2999][0], orig_last);

        // Verify table is now reversed
        assert_eq!(table.get_cell(0, 0), Some(&orig_last));
        assert_eq!(table.get_cell(2999, 0), Some(&orig_first));
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
    fn test_fill_rows_bulk() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
        ]);

        table.fill_rows_with_data_bulk(0, vec![
            vec!["x".to_string(), "7".to_string()],
            vec!["y".to_string(), "8".to_string()],
        ]);

        assert_eq!(row(&table, 0), vec!["x", "7"]);
        assert_eq!(row(&table, 1), vec!["y", "8"]);
        assert_eq!(row(&table, 2), vec!["c", "3"]);
    }

    #[test]
    fn test_fill_rows_bulk_expands_table() {
        let mut table = make_table(vec![
            vec!["a", "1"],
        ]);

        table.fill_rows_with_data_bulk(0, vec![
            vec!["x".to_string(), "7".to_string()],
            vec!["y".to_string(), "8".to_string()],
            vec!["z".to_string(), "9".to_string()],
        ]);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["x", "7"]);
        assert_eq!(row(&table, 1), vec!["y", "8"]);
        assert_eq!(row(&table, 2), vec!["z", "9"]);
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
        assert_eq!(table.probe_row_type(0, true), SortType::Text);
        // Row 1 is numeric (scores)
        assert_eq!(table.probe_row_type(1, true), SortType::Numeric);
    }
}
