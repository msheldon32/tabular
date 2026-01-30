use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

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

    pub fn insert_row_with_data(&mut self, idx: usize, mut row: Vec<String>) {
        row.resize(self.col_count(), String::new());
        self.cells.insert(idx, row);
    }

    pub fn insert_col_with_data(&mut self, idx: usize, col: Vec<String>) {
        for (row, value) in self.cells.iter_mut().zip(col.iter()) {
            if idx <= row.len() {
                row.insert(idx, value.clone());
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

    pub fn paste_row_below(&mut self, table: &mut Table, row: Vec<String>) {
        table.insert_row_with_data(self.cursor_row + 1, row);
        self.cursor_row += 1;
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
        table.get_col(self.cursor_col)
    }

    pub fn paste_col_after(&mut self, table: &mut Table, col: Vec<String>) {
        table.insert_col_with_data(self.cursor_col + 1, col);
        self.update_col_widths(table);
    }
}

impl Default for TableView {
    fn default() -> Self {
        Self::new()
    }
}
