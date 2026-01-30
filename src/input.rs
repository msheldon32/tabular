use std::time::{Duration, Instant};

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

/// Actions resulting from key sequences
#[derive(Clone, Debug, PartialEq)]
pub enum SequenceAction {
    MoveToTop,   // gg
    DeleteRow,   // dr
    DeleteCol,   // dc
    YankRow,     // yr
    YankCol,     // yc
    Yank,        // yy (visual mode)
    Delete,      // dd (visual mode)
    // Motion actions (can have count)
    MoveDown,    // j
    MoveUp,      // k
    MoveLeft,    // h
    MoveRight,   // l
}

/// Result of processing a key through the buffer
pub enum KeyBufferResult {
    /// A sequence matched, execute this action with optional count
    Action(SequenceAction, usize),
    /// Waiting for more keys (buffer is a valid prefix)
    Pending,
    /// No sequence matched, process this key normally (with optional count)
    Fallthrough(KeyEvent, usize),
}

/// Buffer for accumulating multi-key sequences with optional count prefix
pub struct KeyBuffer {
    keys: Vec<char>,
    count: Option<usize>,
    last_key_time: Instant,
    timeout: Duration,
}

impl KeyBuffer {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            count: None,
            last_key_time: Instant::now(),
            timeout: Duration::from_millis(1000),
        }
    }

    /// Process a key event, returning what action to take
    pub fn process(&mut self, key: KeyEvent) -> KeyBufferResult {
        // Clear buffer if too much time has passed since last key
        if self.last_key_time.elapsed() > self.timeout {
            self.keys.clear();
            self.count = None;
        }

        // Only buffer character keys (no modifiers except shift)
        let c = match key.code {
            KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => c,
            _ => {
                // Non-char key breaks any sequence
                let count = self.take_count();
                self.keys.clear();
                return KeyBufferResult::Fallthrough(key, count);
            }
        };

        self.last_key_time = Instant::now();

        // Accumulate digits as count prefix (but not '0' at start - that's a motion)
        if c.is_ascii_digit() && (self.count.is_some() || c != '0' || !self.keys.is_empty()) {
            if self.keys.is_empty() {
                // Still in count prefix phase
                let digit = c.to_digit(10).unwrap() as usize;
                self.count = Some(self.count.unwrap_or(0) * 10 + digit);
                return KeyBufferResult::Pending;
            }
        }

        self.keys.push(c);

        // Try to match a complete sequence
        if let Some(action) = self.match_sequence() {
            let count = self.take_count();
            self.keys.clear();
            return KeyBufferResult::Action(action, count);
        }

        // Check if current buffer could be a prefix of any sequence
        if self.is_valid_prefix() {
            return KeyBufferResult::Pending;
        }

        // No match and not a valid prefix - clear and fall through
        let count = self.take_count();
        self.keys.clear();
        KeyBufferResult::Fallthrough(key, count)
    }

    /// Clear the buffer (e.g., on mode change)
    pub fn clear(&mut self) {
        self.keys.clear();
        self.count = None;
    }

    fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    fn match_sequence(&self) -> Option<SequenceAction> {
        match self.keys.as_slice() {
            ['g', 'g'] => Some(SequenceAction::MoveToTop),
            ['d', 'r'] => Some(SequenceAction::DeleteRow),
            ['d', 'c'] => Some(SequenceAction::DeleteCol),
            ['d', 'd'] => Some(SequenceAction::Delete),
            ['y', 'r'] => Some(SequenceAction::YankRow),
            ['y', 'c'] => Some(SequenceAction::YankCol),
            ['y', 'y'] => Some(SequenceAction::Yank),
            ['j'] => Some(SequenceAction::MoveDown),
            ['k'] => Some(SequenceAction::MoveUp),
            ['h'] => Some(SequenceAction::MoveLeft),
            ['l'] => Some(SequenceAction::MoveRight),
            _ => None,
        }
    }

    fn is_valid_prefix(&self) -> bool {
        matches!(self.keys.as_slice(), ['g'] | ['d'] | ['y'])
    }
}

/// Navigation handler shared across modes
pub struct NavigationHandler;

impl NavigationHandler {
    pub fn new() -> Self {
        Self
    }

    /// Handle navigation keys, returns true if the key was handled
    pub fn handle(&self, key: KeyEvent, view: &mut TableView, table: &Table) -> bool {
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => { view.move_left(); true }
            KeyCode::Char('j') | KeyCode::Down => { view.move_down(table); true }
            KeyCode::Char('k') | KeyCode::Up => { view.move_up(); true }
            KeyCode::Char('l') | KeyCode::Right => { view.move_right(table); true }
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
        nav: &NavigationHandler,
        key_buffer: &mut KeyBuffer,
    ) -> KeyResult {
        if is_escape(key) {
            key_buffer.clear();
            return KeyResult::Finish;
        }

        // Process through key buffer for sequences
        match key_buffer.process(key) {
            KeyBufferResult::Action(action, count) => {
                match action {
                    SequenceAction::MoveToTop => view.move_to_top(),
                    SequenceAction::MoveDown => view.move_down_n(count, table),
                    SequenceAction::MoveUp => view.move_up_n(count),
                    SequenceAction::MoveLeft => view.move_left_n(count),
                    SequenceAction::MoveRight => view.move_right_n(count, table),
                    SequenceAction::Yank => return self.handle_yank(view, table, clipboard),
                    SequenceAction::Delete => return self.handle_delete(view, table),
                    _ => {} // dr, dc, yr, yc not used in visual mode
                }
                KeyResult::Continue
            }
            KeyBufferResult::Pending => {
                KeyResult::Continue
            }
            KeyBufferResult::Fallthrough(key, _count) => {
                // Handle navigation (already handled by KeyBuffer for hjkl)
                nav.handle(key, view, table);

                match key.code {
                    KeyCode::Char('x') => self.handle_delete(view, table),
                    KeyCode::Char(':') => KeyResult::SwitchMode(crate::mode::Mode::Command),
                    KeyCode::Char('q') => self.handle_drag_down(view, table),
                    KeyCode::Char('Q') => self.handle_drag_right(view, table),
                    _ => KeyResult::Continue,
                }
            }
        }
    }

    fn handle_yank(&self, view: &mut TableView, table: &Table, clipboard: &mut Clipboard) -> KeyResult {
        let (start_row, end_row, start_col, end_col) = view.get_selection_bounds();

        match self.visual_type {
            VisualType::Cell => {
                if let Some(span) = table.get_span(start_row, end_row, start_col, end_col) {
                    clipboard.yank_span(span);
                }
            }
            VisualType::Row => {
                // Yank all selected rows
                let rows: Vec<Vec<String>> = (start_row..=end_row)
                    .filter_map(|r| table.get_row(r))
                    .collect();
                if !rows.is_empty() {
                    clipboard.yank_rows(rows);
                }
            }
            VisualType::Col => {
                // Yank all selected columns
                let cols: Vec<Vec<String>> = (0..table.row_count())
                    .map(|r| {
                        (start_col..=end_col)
                            .map(|c| table.get_cell(r, c).cloned().unwrap_or_default())
                            .collect()
                    })
                    .collect();
                if !cols.is_empty() {
                    clipboard.yank_cols(cols);
                }
            }
        }
        KeyResult::Finish
    }

    fn handle_delete(&self, view: &TableView, table: &Table) -> KeyResult {
        let (start_row, end_row, start_col, end_col) = view.get_selection_bounds();

        match self.visual_type {
            VisualType::Cell => {
                // Clear cell contents
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
            VisualType::Row => {
                // Delete entire rows
                let txns: Vec<Transaction> = (start_row..=end_row)
                    .filter_map(|r| {
                        table.get_row(r).map(|data| Transaction::DeleteRow {
                            idx: start_row, // Always delete at start_row since indices shift
                            data,
                        })
                    })
                    .collect();
                if txns.is_empty() {
                    KeyResult::Finish
                } else {
                    KeyResult::ExecuteAndFinish(Transaction::Batch(txns))
                }
            }
            VisualType::Col => {
                // Delete entire columns
                let txns: Vec<Transaction> = (start_col..=end_col)
                    .filter_map(|c| {
                        table.get_col(c).map(|data| Transaction::DeleteCol {
                            idx: start_col, // Always delete at start_col since indices shift
                            data,
                        })
                    })
                    .collect();
                if txns.is_empty() {
                    KeyResult::Finish
                } else {
                    KeyResult::ExecuteAndFinish(Transaction::Batch(txns))
                }
            }
        }
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
