//! Table operations that coordinate between TableView and Table
//!
//! These functions handle operations that need to modify both the table data
//! and the view state (cursor position, selection, etc.)

use crate::table::Table;
use crate::tableview::TableView;

// === Cell Access ===
/// Get current cell content
pub fn current_cell<'a>(view: &TableView, table: &'a Table) -> &'a String {
    table.get_cell(view.cursor_row, view.cursor_col)
        .expect("Cursor should be within bounds")
}
