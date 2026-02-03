use crossterm::event::{KeyCode, KeyEvent};

use crate::table::tableview::TableView;
use crate::table::table::Table;
use crate::input::{KeyResult, is_escape};

/// Search state and functionality
pub struct SearchHandler {
    pub pattern: Option<String>,
    pub matches: Vec<(usize, usize)>,
    pub index: usize,
    pub buffer: String,
}

impl SearchHandler {
    pub fn new() -> Self {
        Self {
            pattern: None,
            matches: Vec::new(),
            index: 0,
            buffer: String::new(),
        }
    }

    pub fn start_search(&mut self) {
        self.buffer.clear();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult {
        if is_escape(key) {
            self.buffer.clear();
            return KeyResult::Finish;
        }

        match key.code {
            KeyCode::Enter => {
                if !self.buffer.is_empty() {
                    self.pattern = Some(self.buffer.clone());
                }
                KeyResult::Finish
            }
            KeyCode::Backspace => {
                self.buffer.pop();
                KeyResult::Continue
            }
            KeyCode::Char(c) => {
                self.buffer.push(c);
                KeyResult::Continue
            }
            _ => KeyResult::Continue
        }
    }

    pub fn perform_search(&mut self, table: &Table) -> Option<String> {
        self.matches.clear();
        self.index = 0;

        let pattern = self.pattern.as_ref()?;
        let pattern_lower = pattern.to_lowercase();

        for row in 0..table.row_count() {
            for col in 0..table.col_count() {
                if let Some(cell) = table.get_cell(row, col) {
                    if cell.to_lowercase().contains(&pattern_lower) {
                        self.matches.push((row, col));
                    }
                }
            }
        }

        if self.matches.is_empty() {
            Some(format!("Pattern not found: {}", pattern))
        } else {
            Some(format!("{} match(es) found", self.matches.len()))
        }
    }

    pub fn goto_next(&mut self, view: &mut TableView) -> Option<String> {
        if self.matches.is_empty() {
            return if self.pattern.is_some() {
                Some("No matches".to_string())
            } else {
                None
            };
        }

        let current_pos = (view.cursor_row, view.cursor_col);
        let mut next_index = None;

        for (i, &(row, col)) in self.matches.iter().enumerate() {
            if (row, col) > current_pos {
                next_index = Some(i);
                break;
            }
        }

        let index = next_index.unwrap_or(0);
        self.index = index;

        let (row, col) = self.matches[index];
        view.cursor_row = row;
        view.cursor_col = col;
        view.scroll_to_cursor();

        Some(format!("[{}/{}] matches", index + 1, self.matches.len()))
    }

    pub fn goto_prev(&mut self, view: &mut TableView) -> Option<String> {
        if self.matches.is_empty() {
            return if self.pattern.is_some() {
                Some("No matches".to_string())
            } else {
                None
            };
        }

        let current_pos = (view.cursor_row, view.cursor_col);
        let mut prev_index = None;

        for (i, &(row, col)) in self.matches.iter().enumerate().rev() {
            if (row, col) < current_pos {
                prev_index = Some(i);
                break;
            }
        }

        let index = prev_index.unwrap_or(self.matches.len() - 1);
        self.index = index;

        let (row, col) = self.matches[index];
        view.cursor_row = row;
        view.cursor_col = col;
        view.scroll_to_cursor();

        Some(format!("[{}/{}] matches", index + 1, self.matches.len()))
    }
}

