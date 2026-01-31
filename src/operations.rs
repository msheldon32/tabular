//! Table operations that coordinate between TableView and Table
//!
//! These functions handle operations that need to modify both the table data
//! and the view state (cursor position, selection, etc.)

use crate::table::Table;
use crate::tableview::TableView;
use crate::util::translate_references;

// === Row Operations ===

pub fn insert_row_below(view: &mut TableView, table: &mut Table) {
    table.insert_row_at(view.cursor_row + 1);
    view.cursor_row += 1;
    view.scroll_to_cursor();
}

pub fn insert_row_above(view: &mut TableView, table: &mut Table) {
    table.insert_row_at(view.cursor_row);
    view.scroll_to_cursor();
}

pub fn delete_row(view: &mut TableView, table: &mut Table) -> Option<Vec<String>> {
    let row = table.delete_row_at(view.cursor_row);
    view.clamp_cursor(table);
    view.scroll_to_cursor();
    row
}

/// Delete multiple selected rows (for VisualRow mode), returns deleted rows
pub fn delete_rows_bulk(view: &mut TableView, table: &mut Table) -> Vec<Vec<String>> {
    let (start_row, end_row, _, _) = view.get_selection_bounds();
    let count = end_row - start_row + 1;
    let deleted = table.delete_rows_bulk(start_row, count);
    view.cursor_row = start_row;
    view.support_row = start_row;
    view.clamp_cursor(table);
    view.scroll_to_cursor();
    deleted
}

pub fn yank_row(view: &TableView, table: &Table) -> Option<Vec<String>> {
    table.get_row_cloned(view.cursor_row)
}

/// Yank multiple selected rows (for VisualRow mode)
pub fn yank_rows_bulk(view: &TableView, table: &Table) -> Vec<Vec<String>> {
    let (start_row, end_row, _, _) = view.get_selection_bounds();
    let count = end_row - start_row + 1;
    table.get_rows_cloned(start_row, count)
}

pub fn paste_row(view: &TableView, table: &mut Table, row: Vec<String>) {
    table.fill_row_with_data(view.cursor_row, row);
}

/// Paste multiple rows starting at cursor, overwriting existing rows
pub fn paste_rows_bulk(view: &TableView, table: &mut Table, rows: Vec<Vec<String>>) {
    table.fill_rows_with_data_bulk(view.cursor_row, rows);
}

/// Insert multiple rows below current selection with data (e.g., after paste)
pub fn insert_rows_below_bulk(view: &mut TableView, table: &mut Table, rows: Vec<Vec<String>>) {
    let count = rows.len();
    let insert_at = view.cursor_row + 1;
    table.insert_rows_with_data_bulk(insert_at, rows);
    view.cursor_row = insert_at;
    view.support_row = insert_at + count - 1;
    view.scroll_to_cursor();
}

/// Insert multiple empty rows below cursor
pub fn insert_rows_below_empty(view: &mut TableView, table: &mut Table, count: usize) {
    let insert_at = view.cursor_row + 1;
    table.insert_rows_bulk(insert_at, count);
    view.cursor_row = insert_at;
    view.support_row = insert_at + count - 1;
    view.scroll_to_cursor();
}

/// Insert multiple rows above current selection with data
pub fn insert_rows_above_bulk(view: &mut TableView, table: &mut Table, rows: Vec<Vec<String>>) {
    let count = rows.len();
    table.insert_rows_with_data_bulk(view.cursor_row, rows);
    view.support_row = view.cursor_row + count - 1;
    view.scroll_to_cursor();
}

// === Column Operations ===

pub fn insert_col_after(view: &TableView, table: &mut Table) {
    table.insert_col_at(view.cursor_col + 1);
}

pub fn delete_col(view: &mut TableView, table: &mut Table) -> Option<Vec<String>> {
    let col = table.delete_col_at(view.cursor_col);
    view.clamp_cursor(table);
    view.scroll_to_cursor();
    col
}

pub fn yank_col(view: &TableView, table: &Table) -> Option<Vec<String>> {
    table.get_col_cloned(view.cursor_col)
}

pub fn paste_col(view: &TableView, table: &mut Table, col: Vec<String>) {
    table.fill_col_with_data(view.cursor_col, col);
}

// === Span Operations ===

pub fn yank_span(view: &TableView, table: &Table) -> Option<Vec<Vec<String>>> {
    let (start_row, end_row, start_col, end_col) = view.get_selection_bounds();
    table.get_span(start_row, end_row, start_col, end_col)
}

pub fn paste_span(view: &TableView, table: &mut Table, span: Vec<Vec<String>>) {
    table.fill_span_with_data(view.cursor_row, view.cursor_col, span);
}

pub fn clear_span(view: &TableView, table: &mut Table) {
    let (start_row, end_row, start_col, end_col) = view.get_selection_bounds();
    for row_idx in start_row..=end_row {
        for col_idx in start_col..=end_col {
            table.set_cell(row_idx, col_idx, String::new());
        }
    }
}

pub fn clear_row_span(view: &TableView, table: &mut Table) {
    let (start_row, end_row, _, _) = view.get_selection_bounds();
    let col_count = table.col_count();
    for row_idx in start_row..=end_row {
        for col_idx in 0..col_count {
            table.set_cell(row_idx, col_idx, String::new());
        }
    }
}

pub fn clear_col_span(view: &TableView, table: &mut Table) {
    let (_, _, start_col, end_col) = view.get_selection_bounds();
    let row_count = table.row_count();
    for row_idx in 0..row_count {
        for col_idx in start_col..=end_col {
            table.set_cell(row_idx, col_idx, String::new());
        }
    }
}

// === Drag Operations (fill with reference translation) ===

pub fn drag_down(view: &TableView, table: &mut Table, whole_row: bool) {
    let (start_row, end_row, sel_start_col, sel_end_col) = view.get_selection_bounds();
    let (start_col, end_col) = if whole_row {
        (0, table.col_count() - 1)
    } else {
        (sel_start_col, sel_end_col)
    };

    for row_idx in start_row + 1..=end_row {
        for col_idx in start_col..=end_col {
            let source = table.get_cell(start_row, col_idx).cloned().unwrap_or_default();
            let new_val = translate_references(&source, (row_idx - start_row) as isize, 0isize);
            table.set_cell(row_idx, col_idx, new_val);
        }
    }
}

pub fn drag_up(view: &TableView, table: &mut Table, whole_row: bool) {
    let (start_row, end_row, sel_start_col, sel_end_col) = view.get_selection_bounds();
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

pub fn drag_right(view: &TableView, table: &mut Table, whole_col: bool) {
    let (sel_start_row, sel_end_row, start_col, end_col) = view.get_selection_bounds();
    let (start_row, end_row) = if whole_col {
        (0, table.row_count() - 1)
    } else {
        (sel_start_row, sel_end_row)
    };

    for row_idx in start_row..=end_row {
        for col_idx in start_col + 1..=end_col {
            let source = table.get_cell(row_idx, start_col).cloned().unwrap_or_default();
            let new_val = translate_references(&source, 0isize, (col_idx - start_col) as isize);
            table.set_cell(row_idx, col_idx, new_val);
        }
    }
}

pub fn drag_left(view: &TableView, table: &mut Table, whole_col: bool) {
    let (sel_start_row, sel_end_row, start_col, end_col) = view.get_selection_bounds();
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

// === Cell Access ===

/// Get current cell content
pub fn current_cell<'a>(view: &TableView, table: &'a Table) -> &'a String {
    table.get_cell(view.cursor_row, view.cursor_col)
        .expect("Cursor should be within bounds")
}

/// Get mutable reference to current cell
pub fn current_cell_mut<'a>(view: &TableView, table: &'a mut Table) -> &'a mut String {
    table.get_row_mut(view.cursor_row)
        .and_then(|r| r.get_mut(view.cursor_col))
        .expect("Cursor should be within bounds")
}
