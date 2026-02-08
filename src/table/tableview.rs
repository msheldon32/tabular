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
        } else if self.row_manager.borrow().should_scroll(self.cursor_row, self.viewport_row, self.viewport_height) {
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
