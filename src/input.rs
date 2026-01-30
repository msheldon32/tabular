use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::clipboard::Clipboard;
use crate::table::{Table, TableView};
use crate::transaction::Transaction;

/// Result of handling a key event
#[allow(dead_code)]
pub enum KeyResult {
    /// Continue in current mode
    Continue,
    /// Switch to a different mode
    SwitchMode(crate::mode::Mode),
    /// Execute a transaction
    Execute(Transaction),
    /// Execute a transaction and return to normal mode
    ExecuteAndFinish(Transaction),
    /// Return to normal mode
    Finish,
    /// Show a message
    Message(String),
    /// Quit the application
    Quit,
    /// Force quit
    ForceQuit,
}

/// Check for escape key (Esc or Ctrl+[)
pub fn is_escape(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL))
}

/// Navigation handler shared across modes
pub struct NavigationHandler {
    pending_key: Option<char>,
}

impl NavigationHandler {
    pub fn new() -> Self {
        Self { pending_key: None }
    }

    pub fn pending_key(&self) -> Option<char> {
        self.pending_key
    }

    pub fn set_pending_key(&mut self, key: Option<char>) {
        self.pending_key = key;
    }

    /// Handle navigation keys, returns true if the key was handled
    pub fn handle(&mut self, key: KeyEvent, view: &mut TableView, table: &Table) -> bool {
        // Handle pending 'gg' sequence
        if let Some(pending) = self.pending_key.take() {
            if pending == 'g' && key.code == KeyCode::Char('g') {
                view.move_to_top();
                return true;
            }
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => { view.move_left(); true }
            KeyCode::Char('j') | KeyCode::Down => { view.move_down(table); true }
            KeyCode::Char('k') | KeyCode::Up => { view.move_up(); true }
            KeyCode::Char('l') | KeyCode::Right => { view.move_right(table); true }
            KeyCode::Char('g') => { self.pending_key = Some('g'); true }
            KeyCode::Char('G') => { view.move_to_bottom(table); true }
            KeyCode::Char('0') | KeyCode::Char('^') => { view.move_to_first_col(); true }
            KeyCode::Char('$') => { view.move_to_last_col(table); true }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                view.half_page_down(table); true
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                view.half_page_up(); true
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                view.page_down(table); true
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                view.page_up(); true
            }
            _ => false
        }
    }
}

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

/// Insert mode handler
pub struct InsertHandler {
    pub buffer: String,
    pub cursor: usize,
}

impl InsertHandler {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
        }
    }

    pub fn start_edit(&mut self, initial: String) {
        self.buffer = initial;
        self.cursor = self.buffer.len();
    }

    pub fn handle_key(&mut self, key: KeyEvent, view: &TableView) -> (KeyResult, Option<Transaction>) {
        if is_escape(key) || key.code == KeyCode::Enter {
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
                    self.buffer.remove(self.cursor);
                }
            }
            KeyCode::Char(c) => {
                self.buffer.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                self.cursor = std::cmp::min(self.cursor + 1, self.buffer.len());
            }
            _ => {}
        }

        (KeyResult::Continue, None)
    }
}

/// Visual selection mode types
#[derive(Clone, Copy, PartialEq)]
pub enum VisualType {
    Cell,
    Row,
    Col,
}

/// Unified visual mode handler
pub struct VisualHandler {
    pub visual_type: VisualType,
}

impl VisualHandler {
    pub fn new(visual_type: VisualType) -> Self {
        Self { visual_type }
    }

    /// Handle a key event in visual mode
    pub fn handle_key(
        &self,
        key: KeyEvent,
        view: &mut TableView,
        table: &Table,
        clipboard: &mut Clipboard,
        nav: &mut NavigationHandler,
    ) -> KeyResult {
        if is_escape(key) {
            return KeyResult::Finish;
        }

        // Handle navigation
        nav.handle(key, view, table);

        match key.code {
            KeyCode::Char('y') => self.handle_yank(view, table, clipboard),
            KeyCode::Char('x') => self.handle_delete(view, table),
            KeyCode::Char(':') => KeyResult::SwitchMode(crate::mode::Mode::Command),
            KeyCode::Char('q') => self.handle_drag_down(view, table),
            KeyCode::Char('Q') => self.handle_drag_right(view, table),
            _ => KeyResult::Continue,
        }
    }

    fn handle_yank(&self, view: &mut TableView, table: &Table, clipboard: &mut Clipboard) -> KeyResult {
        match self.visual_type {
            VisualType::Cell => {
                if let Some(span) = view.yank_span(table) {
                    clipboard.yank_span(span);
                }
            }
            VisualType::Row => {
                if let Some(row) = view.yank_row(table) {
                    clipboard.yank_row(row);
                }
            }
            VisualType::Col => {
                if let Some(col) = view.yank_col(table) {
                    clipboard.yank_col(col);
                }
            }
        }
        KeyResult::Finish
    }

    fn handle_delete(&self, view: &TableView, table: &Table) -> KeyResult {
        let (sr, er, sc, ec) = view.get_selection_bounds();
        let (start_row, end_row, start_col, end_col) = match self.visual_type {
            VisualType::Cell => (sr, er, sc, ec),
            VisualType::Row => (sr, er, 0, table.col_count() - 1),
            VisualType::Col => (0, table.row_count() - 1, sc, ec),
        };

        let old_data = table.get_span(start_row, end_row, start_col, end_col)
            .unwrap_or_default();
        let new_data = vec![vec![String::new(); end_col - start_col + 1]; end_row - start_row + 1];

        let txn = Transaction::SetSpan {
            row: start_row,
            col: start_col,
            old_data,
            new_data,
        };
        KeyResult::ExecuteAndFinish(txn)
    }

    fn handle_drag_down(&self, view: &TableView, table: &Table) -> KeyResult {
        match self.visual_type {
            VisualType::Cell | VisualType::Row => {
                let txn = create_drag_down_txn(view, table, self.visual_type == VisualType::Row);
                KeyResult::ExecuteAndFinish(txn)
            }
            VisualType::Col => KeyResult::Continue, // Not applicable
        }
    }

    fn handle_drag_right(&self, view: &TableView, table: &Table) -> KeyResult {
        match self.visual_type {
            VisualType::Cell | VisualType::Col => {
                let txn = create_drag_right_txn(view, table, self.visual_type == VisualType::Col);
                KeyResult::ExecuteAndFinish(txn)
            }
            VisualType::Row => KeyResult::Continue, // Not applicable
        }
    }
}

/// Create a drag-down transaction (fill formula down)
fn create_drag_down_txn(view: &TableView, table: &Table, whole_row: bool) -> Transaction {
    let (start_row, end_row, mut start_col, mut end_col) = view.get_selection_bounds();
    if whole_row {
        start_col = 0;
        end_col = table.col_count() - 1;
    }

    let old_data = table.get_span(start_row, end_row, start_col, end_col)
        .unwrap_or_default();

    let mut new_data = old_data.clone();
    for row_idx in 1..new_data.len() {
        for col_idx in 0..new_data[row_idx].len() {
            new_data[row_idx][col_idx] = crate::util::translate_references(
                &new_data[0][col_idx],
                row_idx as isize,
                0,
            );
        }
    }

    Transaction::SetSpan {
        row: start_row,
        col: start_col,
        old_data,
        new_data,
    }
}

/// Create a drag-right transaction (fill formula right)
fn create_drag_right_txn(view: &TableView, table: &Table, whole_col: bool) -> Transaction {
    let (mut start_row, mut end_row, start_col, end_col) = view.get_selection_bounds();
    if whole_col {
        start_row = 0;
        end_row = table.row_count() - 1;
    }

    let old_data = table.get_span(start_row, end_row, start_col, end_col)
        .unwrap_or_default();

    let mut new_data = old_data.clone();
    for row_idx in 0..new_data.len() {
        for col_idx in 1..new_data[row_idx].len() {
            new_data[row_idx][col_idx] = crate::util::translate_references(
                &new_data[row_idx][0],
                0,
                col_idx as isize,
            );
        }
    }

    Transaction::SetSpan {
        row: start_row,
        col: start_col,
        old_data,
        new_data,
    }
}

/// Command mode handler
pub struct CommandHandler {
    pub buffer: String,
}

impl CommandHandler {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn start(&mut self) {
        self.buffer.clear();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        if is_escape(key) {
            self.buffer.clear();
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                let cmd = self.buffer.clone();
                self.buffer.clear();
                Some(cmd)
            }
            KeyCode::Backspace => {
                self.buffer.pop();
                None
            }
            KeyCode::Char(c) => {
                self.buffer.push(c);
                None
            }
            _ => None,
        }
    }
}
