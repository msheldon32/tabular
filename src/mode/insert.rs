use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::{KeyResult, is_escape};
use crate::transaction::Transaction;
use crate::table::tableview::TableView;
use crate::string::{get_word_start, get_word_end};

/// Insert mode handler
/// Note: cursor is a CHARACTER index, not a byte index
pub struct InsertHandler {
    pub buffer: String,
    /// Cursor position as character index (not byte index)
    pub cursor: usize,

    true_val: String
}

impl InsertHandler {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            true_val: String::new(),
            cursor: 0,
        }
    }

    pub fn start_edit(&mut self, initial: String) {
        self.true_val = initial.clone();
        self.buffer = initial;
        self.cursor = crate::util::char_count(&self.buffer);
    }

    pub fn handle_key(&mut self, key: KeyEvent, view: &TableView) -> (KeyResult, Option<Transaction>) {
        if is_escape(key) {
            self.buffer = self.true_val.clone();
            return (KeyResult::Finish, None);
        }

        if key.code == KeyCode::Enter {
            let txn = Transaction::SetCell {
                row: view.cursor_row,
                col: view.cursor_col,
                old_value: String::new(), // Will be filled by caller
                new_value: self.buffer.clone(),
            };
            return (KeyResult::ExecuteAndFinish(txn), None);
        }

        match key.code {
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    if let Some((new_buf, _)) = crate::util::remove_char_at(&self.buffer, self.cursor) {
                        self.buffer = new_buf;
                    }
                }
            }
            KeyCode::Char(c) => {
                self.buffer = crate::util::insert_char_at(&self.buffer, self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = get_word_start(&self.buffer, self.cursor);
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => { 
                self.cursor = get_word_end(&self.buffer, self.cursor);
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                let char_count = crate::util::char_count(&self.buffer);
                self.cursor = std::cmp::min(self.cursor + 1, char_count);
            }
            _ => {}
        }

        (KeyResult::Continue, None)
    }
}

