use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::cmp;

use crate::mode::Mode;
use crate::util::translate_references;

/// Pure data structure for the table
#[derive(Debug, Clone)]
pub struct Table {
    pub cells: Vec<Vec<String>>,
}

impl Table {
    pub fn new() -> Self {
        Self {
            cells: vec![vec![String::new()]],
        }
    }

    pub fn load_csv<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);

        let mut cells: Vec<Vec<String>> = Vec::new();

        for result in csv_reader.records() {
            let record = result.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            cells.push(row);
        }

        if cells.is_empty() {
            cells.push(vec![String::new()]);
        }

        Ok(Self { cells })
    }

    pub fn save_csv<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let mut csv_writer = csv::Writer::from_writer(writer);

        for row in &self.cells {
            csv_writer
                .write_record(row)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }

        csv_writer
            .flush()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(())
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Option<&String> {
        self.cells.get(row).and_then(|r| r.get(col))
    }

    pub fn set_cell(&mut self, row: usize, col: usize, value: String) {
        if let Some(r) = self.cells.get_mut(row) {
            if let Some(cell) = r.get_mut(col) {
                *cell = value;
            }
        }
    }

    pub fn row_count(&self) -> usize {
        self.cells.len()
    }

    pub fn col_count(&self) -> usize {
        self.cells.first().map(|r| r.len()).unwrap_or(0)
    }

    pub fn insert_row_at(&mut self, idx: usize) {
        let new_row = vec![String::new(); self.col_count()];
        self.cells.insert(idx, new_row);
    }

    pub fn delete_row_at(&mut self, idx: usize) -> Option<Vec<String>> {
        if self.row_count() <= 1 {
            let row = self.cells[0].clone();
            self.cells[0] = vec![String::new(); self.col_count()];
            return Some(row);
        }

        if idx < self.row_count() {
            Some(self.cells.remove(idx))
        } else {
            None
        }
    }

    pub fn insert_col_at(&mut self, idx: usize) {
        for row in &mut self.cells {
            if idx <= row.len() {
                row.insert(idx, String::new());
            }
        }
    }

    pub fn delete_col_at(&mut self, idx: usize) -> Option<Vec<String>> {
        if self.col_count() <= 1 {
            let col: Vec<String> = self.cells.iter().map(|r| r[0].clone()).collect();
            for row in &mut self.cells {
                row[0] = String::new();
            }
            return Some(col);
        }

        if idx < self.col_count() {
            let col: Vec<String> = self.cells.iter().map(|r| r[idx].clone()).collect();
            for row in &mut self.cells {
                if idx < row.len() {
                    row.remove(idx);
                }
            }
            Some(col)
        } else {
            None
        }
    }

    pub fn get_row(&self, idx: usize) -> Option<Vec<String>> {
        self.cells.get(idx).cloned()
    }

    pub fn get_col(&self, idx: usize) -> Option<Vec<String>> {
        if idx < self.col_count() {
            Some(self.cells.iter().map(|r| r[idx].clone()).collect())
        } else {
            None
        }
    }

    pub fn get_span(&self, start_row: usize, end_row: usize, start_col: usize, end_col: usize) -> Option<Vec<Vec<String>>> {
        let mut out_vec = Vec::new();

        for row_iter in start_row..=end_row {
            let mut row = Vec::new();
            for col_iter in start_col..=end_col {
                row.push(self.cells[row_iter][col_iter].clone());
            }
            out_vec.push(row);
        }

        Some(out_vec)
    }

    pub fn insert_row_with_data(&mut self, idx: usize, mut row: Vec<String>) {
        row.resize(self.col_count(), String::new());
        self.cells.insert(idx, row);
    }

    pub fn fill_row_with_data(&mut self, idx: usize, row: Vec<String>) {
        if row.len() != self.col_count() {
            return;
        }
        self.cells[idx] = row;
    }

    pub fn insert_col_with_data(&mut self, idx: usize, col: Vec<String>) {
        for (row, value) in self.cells.iter_mut().zip(col.iter()) {
            if idx <= row.len() {
                row.insert(idx, value.clone());
            }
        }
    }

    pub fn fill_col_with_data(&mut self, idx: usize, col: Vec<String>) {
        for (row, value) in self.cells.iter_mut().zip(col.iter()) {
            if idx < row.len() {
                row[idx] = value.clone();
            }
        }
    }

    pub fn fill_span_with_data(&mut self, row_idx: usize, col_idx: usize, span: Vec<Vec<String>>) {
        for (dx, row) in span.iter().enumerate() {
            if row_idx + dx >= self.cells.len() {
                self.insert_row_at(row_idx+dx);
            }
            for (dy, val) in row.iter().enumerate() {
                if col_idx+dy >= self.cells[row_idx+dx].len() {
                    self.insert_col_at(col_idx+dy);
                }
                self.cells[row_idx+dx][col_idx+dy] = val.clone();
            }
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

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
    pub fn update_col_widths(&mut self, table: &Table) {
        self.col_widths = (0..table.col_count())
            .map(|col| {
                table
                    .cells
                    .iter()
                    .filter_map(|row| row.get(col))
                    .map(|s| s.len())
                    .max()
                    .unwrap_or(3)
                    .max(3)
            })
            .collect();
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

    /// Get current cell content
    pub fn current_cell<'a>(&self, table: &'a Table) -> &'a String {
        table.get_cell(self.cursor_row, self.cursor_col)
            .expect("Cursor should be within bounds")
    }

    /// Get mutable reference to current cell
    pub fn current_cell_mut<'a>(&self, table: &'a mut Table) -> &'a mut String {
        &mut table.cells[self.cursor_row][self.cursor_col]
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

    pub fn yank_row(&self, table: &Table) -> Option<Vec<String>> {
        table.get_row(self.cursor_row)
    }

    pub fn paste_row(&mut self, table: &mut Table, row: Vec<String>) {
        table.fill_row_with_data(self.cursor_row, row);
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
        table.get_col(self.cursor_col)
    }

    pub fn paste_col(&mut self, table: &mut Table, col: Vec<String>) {
        table.fill_col_with_data(self.cursor_col, col);
        self.update_col_widths(table);
    }

    pub fn yank_span(&self, table: &Table) -> Option<Vec<Vec<String>>> {
        let start_row = cmp::min(self.cursor_row, self.support_row);
        let end_row = cmp::max(self.cursor_row, self.support_row);
        let start_col = cmp::min(self.cursor_col, self.support_col);
        let end_col = cmp::max(self.cursor_col, self.support_col);

        table.get_span(start_row, end_row, start_col, end_col)
    }

    pub fn paste_span(&mut self, table: &mut Table, span: Vec<Vec<String>>) {
        table.fill_span_with_data(self.cursor_row, self.cursor_col, span);
    }

    pub fn clear_span(&mut self, table: &mut Table) {
        let start_row = cmp::min(self.cursor_row, self.support_row);
        let end_row = cmp::max(self.cursor_row, self.support_row);
        let start_col = cmp::min(self.cursor_col, self.support_col);
        let end_col = cmp::max(self.cursor_col, self.support_col);

        for row_idx in start_row..=end_row {
            for col_idx in start_col..=end_col {
              table.cells[row_idx][col_idx] = String::new();
            }
        }
    }

    pub fn clear_row_span(&mut self, table: &mut Table) {
        let start_row = cmp::min(self.cursor_row, self.support_row);
        let end_row = cmp::max(self.cursor_row, self.support_row);

        for row_idx in start_row..=end_row {
            for col_idx in 0..table.cells[0].len() {
              table.cells[row_idx][col_idx] = String::new();
            }
        }
    }

    pub fn clear_col_span(&mut self, table: &mut Table) {
        let start_col = cmp::min(self.cursor_col, self.support_col);
        let end_col = cmp::max(self.cursor_col, self.support_col);

        for row_idx in 0..table.cells.len() {
            for col_idx in start_col..=end_col {
              table.cells[row_idx][col_idx] = String::new();
            }
        }
    }

    pub fn drag_down(&mut self, table: &mut Table, whole_row: bool) {
        let start_row = cmp::min(self.cursor_row, self.support_row);
        let end_row = cmp::max(self.cursor_row, self.support_row);
        let (start_col, end_col) = if whole_row {
            (0, table.cells[0].len() - 1)
        } else {
            (cmp::min(self.cursor_col, self.support_col), cmp::max(self.cursor_col, self.support_col))
        };

        for row_idx in start_row+1..=end_row {
            for col_idx in start_col..=end_col {
                table.cells[row_idx][col_idx] = translate_references(table.cells[start_row][col_idx].as_str(), (row_idx - start_row) as isize, 0isize);
            }
        }
    }

    pub fn drag_up(&mut self, table: &mut Table, whole_row: bool) {
        let start_row = cmp::min(self.cursor_row, self.support_row);
        let end_row = cmp::max(self.cursor_row, self.support_row);
        let (start_col, end_col) = if whole_row {
            (0, table.cells[0].len() - 1)
        } else {
            (cmp::min(self.cursor_col, self.support_col), cmp::max(self.cursor_col, self.support_col))
        };

        for row_idx in start_row..end_row {
            let offset = row_idx as isize - end_row as isize;
            for col_idx in start_col..=end_col {
                table.cells[row_idx][col_idx] = translate_references(table.cells[end_row][col_idx].as_str(), offset, 0isize);
            }
        }
    }

    pub fn drag_right(&mut self, table: &mut Table, whole_col: bool) {
        let (start_row, end_row) = if whole_col {
            (0, table.cells.len() - 1)
        } else {
            (cmp::min(self.cursor_row, self.support_row), cmp::max(self.cursor_row, self.support_row))
        };

        let start_col = cmp::min(self.cursor_col, self.support_col);
        let end_col = cmp::max(self.cursor_col, self.support_col);

        for row_idx in start_row..=end_row {
            for col_idx in start_col+1..=end_col {
                table.cells[row_idx][col_idx] = translate_references(table.cells[row_idx][start_col].as_str(), 0isize, (col_idx - start_col) as isize);
            }
        }
    }

    pub fn drag_left(&mut self, table: &mut Table, whole_col: bool) {
        let (start_row, end_row) = if whole_col {
            (0, table.cells.len() - 1)
        } else {
            (cmp::min(self.cursor_row, self.support_row), cmp::max(self.cursor_row, self.support_row))
        };

        let start_col = cmp::min(self.cursor_col, self.support_col);
        let end_col = cmp::max(self.cursor_col, self.support_col);

        for row_idx in start_row..=end_row {
            for col_idx in start_col..end_col {
                let offset = col_idx as isize - end_col as isize;
                table.cells[row_idx][col_idx] = translate_references(table.cells[row_idx][end_col].as_str(), 0isize, offset);
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

    fn make_table(data: Vec<Vec<&str>>) -> Table {
        Table {
            cells: data.into_iter()
                .map(|row| row.into_iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    // === Table basic operations ===

    #[test]
    fn test_table_new() {
        let table = Table::new();
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.col_count(), 1);
        assert_eq!(table.cells[0][0], "");
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
        assert_eq!(table.cells[0][1], "x");

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
        assert_eq!(table.cells[0], vec!["", ""]);
        assert_eq!(table.cells[1], vec!["a", "b"]);
    }

    #[test]
    fn test_insert_row_at_middle() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(1);

        assert_eq!(table.row_count(), 3);
        assert_eq!(table.cells[0], vec!["a", "b"]);
        assert_eq!(table.cells[1], vec!["", ""]);
        assert_eq!(table.cells[2], vec!["c", "d"]);
    }

    #[test]
    fn test_insert_row_at_end() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(2);

        assert_eq!(table.row_count(), 3);
        assert_eq!(table.cells[2], vec!["", ""]);
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
        assert_eq!(table.cells[0], vec!["a", "b"]);
        assert_eq!(table.cells[1], vec!["e", "f"]);
    }

    #[test]
    fn test_delete_last_row_clears_instead() {
        let mut table = make_table(vec![
            vec!["a", "b"],
        ]);

        let deleted = table.delete_row_at(0);

        assert_eq!(deleted, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.cells[0], vec!["", ""]);
    }

    #[test]
    fn test_get_row() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_row(0), Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.get_row(1), Some(vec!["c".to_string(), "d".to_string()]));
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
        assert_eq!(table.cells[0], vec!["", "a", "b"]);
        assert_eq!(table.cells[1], vec!["", "c", "d"]);
    }

    #[test]
    fn test_insert_col_at_middle() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_at(1);

        assert_eq!(table.col_count(), 3);
        assert_eq!(table.cells[0], vec!["a", "", "b"]);
        assert_eq!(table.cells[1], vec!["c", "", "d"]);
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
        assert_eq!(table.cells[0], vec!["a", "c"]);
        assert_eq!(table.cells[1], vec!["d", "f"]);
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
        assert_eq!(table.cells[0], vec![""]);
        assert_eq!(table.cells[1], vec![""]);
    }

    #[test]
    fn test_get_col() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_col(0), Some(vec!["a".to_string(), "c".to_string()]));
        assert_eq!(table.get_col(1), Some(vec!["b".to_string(), "d".to_string()]));
        assert_eq!(table.get_col(2), None);
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

    // === Insert with data ===

    #[test]
    fn test_insert_row_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_with_data(1, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(table.row_count(), 3);
        assert_eq!(table.cells[1], vec!["x", "y"]);
    }

    #[test]
    fn test_insert_row_with_data_pads_short_row() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
        ]);

        table.insert_row_with_data(1, vec!["x".to_string()]);

        assert_eq!(table.cells[1], vec!["x", "", ""]);
    }

    #[test]
    fn test_insert_col_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_with_data(1, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(table.cells[0], vec!["a", "x", "b"]);
        assert_eq!(table.cells[1], vec!["c", "y", "d"]);
    }

    // === Fill operations ===

    #[test]
    fn test_fill_row_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.fill_row_with_data(0, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(table.cells[0], vec!["x", "y"]);
        assert_eq!(table.cells[1], vec!["c", "d"]);
    }

    #[test]
    fn test_fill_row_with_data_wrong_size_ignored() {
        let mut table = make_table(vec![
            vec!["a", "b"],
        ]);

        table.fill_row_with_data(0, vec!["x".to_string()]); // Wrong size

        assert_eq!(table.cells[0], vec!["a", "b"]); // Unchanged
    }

    #[test]
    fn test_fill_col_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.fill_col_with_data(0, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(table.cells[0], vec!["x", "b"]);
        assert_eq!(table.cells[1], vec!["y", "d"]);
    }

    #[test]
    fn test_fill_span_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        table.fill_span_with_data(0, 0, vec![
            vec!["1".to_string(), "2".to_string()],
            vec!["3".to_string(), "4".to_string()],
        ]);

        assert_eq!(table.cells[0], vec!["1", "2", "c"]);
        assert_eq!(table.cells[1], vec!["3", "4", "f"]);
        assert_eq!(table.cells[2], vec!["g", "h", "i"]);
    }

    // === TableView tests ===

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
}
