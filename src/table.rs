use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Table {
    pub cells: Vec<Vec<String>>,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

impl Table {
    pub fn new() -> Self {
        Self {
            cells: vec![vec![String::new()]],
            cursor_row: 0,
            cursor_col: 0,
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

        Ok(Self {
            cells,
            cursor_row: 0,
            cursor_col: 0,
        })
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

    pub fn current_cell(&self) -> &String {
        self.get_cell(self.cursor_row, self.cursor_col)
            .expect("Cursor should always be within bounds")
    }

    pub fn current_cell_mut(&mut self) -> &mut String {
        &mut self.cells[self.cursor_row][self.cursor_col]
    }

    pub fn row_count(&self) -> usize {
        self.cells.len()
    }

    pub fn col_count(&self) -> usize {
        self.cells.first().map(|r| r.len()).unwrap_or(0)
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_col + 1 < self.col_count() {
            self.cursor_col += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.row_count() {
            self.cursor_row += 1;
        }
    }

    pub fn insert_row_below(&mut self) {
        let new_row = vec![String::new(); self.col_count()];
        self.cells.insert(self.cursor_row + 1, new_row);
        self.cursor_row += 1;
    }

    pub fn insert_row_above(&mut self) {
        let new_row = vec![String::new(); self.col_count()];
        self.cells.insert(self.cursor_row, new_row);
    }

    pub fn delete_row(&mut self) -> Option<Vec<String>> {
        if self.row_count() <= 1 {
            // Don't delete the last row, just clear it
            let row = self.cells[0].clone();
            self.cells[0] = vec![String::new(); self.col_count()];
            return Some(row);
        }

        let row = self.cells.remove(self.cursor_row);

        // Adjust cursor if we deleted the last row
        if self.cursor_row >= self.row_count() {
            self.cursor_row = self.row_count() - 1;
        }

        Some(row)
    }

    pub fn yank_row(&self) -> Vec<String> {
        self.cells[self.cursor_row].clone()
    }

    pub fn yank_column(&self) -> Vec<String> {
        let mut out = Vec::new();
        for row in self.cells.iter() {
            out.push(row[self.cursor_col].clone());
        }

        out
    }

    pub fn paste_row_below(&mut self, row: Vec<String>) {
        // Ensure the pasted row has the correct number of columns
        let mut row = row;
        row.resize(self.col_count(), String::new());
        self.cells.insert(self.cursor_row + 1, row);
        self.cursor_row += 1;
    }

    pub fn paste_column_after(&mut self, col: Vec<String>) {
        for (row, v) in self.cells.iter_mut().zip(col.iter()) {
            //row.push(String::new());
            row.insert(self.cursor_col + 1, v.clone());
        }
    }


    pub fn add_column_after(&mut self) {
        for row in &mut self.cells {
            //row.push(String::new());
            row.insert(self.cursor_col + 1, String::new());
        }
    }

    pub fn delete_column(&mut self) -> Option<Vec<String>> {
        let col = self.yank_column();

        if self.col_count() <= 1 {
            // Don't delete the last column, just clear it
            for row in &mut self.cells {
                row[0] = String::new();
            }
            return Some(col);
        }

        for row in &mut self.cells {
            if self.cursor_col < row.len() {
                row.remove(self.cursor_col);
            }
        }

        // Adjust cursor if we deleted the last column
        if self.cursor_col >= self.col_count() {
            self.cursor_col = self.col_count() - 1;
        }

        Some(col)
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}
