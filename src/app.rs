use std::cmp;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, poll, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::calculator::Calculator;
use crate::clipboard::Clipboard;
use crate::command::Command;
use crate::mode::Mode;
use crate::table::{Table, TableView};
use crate::transaction::{History, Transaction};
use crate::ui;

pub struct App {
    pub table: Table,
    pub view: TableView,
    pub clipboard: Clipboard,
    pub history: History,
    pub mode: Mode,
    pub command_buffer: String,
    pub edit_buffer: String,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub message: Option<String>,
    pub should_quit: bool,
    pub pending_key: Option<char>,
    pub header_mode: bool,
}

impl App {
    pub fn new(file_path: Option<PathBuf>) -> io::Result<Self> {
        let table = if let Some(ref path) = file_path {
            Table::load_csv(path)?
        } else {
            Table::new()
        };

        let mut view = TableView::new();
        view.update_col_widths(&table);

        Ok(Self {
            table,
            view,
            clipboard: Clipboard::new(),
            history: History::new(),
            mode: Mode::Normal,
            command_buffer: String::new(),
            edit_buffer: String::new(),
            file_path,
            dirty: false,
            message: None,
            should_quit: false,
            pending_key: None,
            header_mode: true,
        })
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        self.view.update_col_widths(&self.table);

        while !self.should_quit {
            terminal.draw(|f| ui::render(f, self))?;

            if poll(Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    self.message = None;
                    self.handle_key(key);
                }
            }
        }
        Ok(())
    }

    // === Transaction helpers ===

    /// Execute a transaction, record it in history, and mark dirty
    fn execute(&mut self, txn: Transaction) {
        txn.apply(&mut self.table);
        self.history.record(txn);
        self.dirty = true;
    }

    /// Execute and return to normal mode
    fn execute_and_finish(&mut self, txn: Transaction) {
        self.execute(txn);
        self.finish_edit();
    }

    /// Return to normal mode and update column widths
    fn finish_edit(&mut self) {
        self.mode = Mode::Normal;
        self.view.update_col_widths(&self.table);
    }

    /// Check for escape key (Esc or Ctrl+[)
    fn is_escape(key: KeyEvent) -> bool {
        key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL))
    }

    // === Span helpers ===

    fn selection_bounds(&self) -> (usize, usize, usize, usize) {
        let start_row = cmp::min(self.view.cursor_row, self.view.support_row);
        let end_row = cmp::max(self.view.cursor_row, self.view.support_row);
        let start_col = cmp::min(self.view.cursor_col, self.view.support_col);
        let end_col = cmp::max(self.view.cursor_col, self.view.support_col);
        (start_row, end_row, start_col, end_col)
    }

    fn create_clear_span_txn(&self, start_row: usize, end_row: usize, start_col: usize, end_col: usize) -> Transaction {
        let old_data = self.table.get_span(start_row, end_row, start_col, end_col)
            .unwrap_or_default();
        let new_data = vec![vec![String::new(); end_col - start_col + 1]; end_row - start_row + 1];
        Transaction::SetSpan {
            row: start_row,
            col: start_col,
            old_data,
            new_data,
        }
    }

    fn create_drag_down_txn(&self, whole_row: bool) -> Transaction {
        let (start_row, end_row, mut start_col, mut end_col) = self.selection_bounds();
        if whole_row {
            start_col = 0;
            end_col = self.table.col_count() - 1;
        }

        let old_data = self.table.get_span(start_row, end_row, start_col, end_col)
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

    fn create_drag_right_txn(&self, whole_col: bool) -> Transaction {
        let (mut start_row, mut end_row, start_col, end_col) = self.selection_bounds();
        if whole_col {
            start_row = 0;
            end_row = self.table.row_count() - 1;
        }

        let old_data = self.table.get_span(start_row, end_row, start_col, end_col)
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

    // === Key handling ===

    fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Command => self.handle_command_mode(key),
            Mode::Visual => self.handle_visual_mode(key),
            Mode::VisualRow => self.handle_visual_row_mode(key),
            Mode::VisualCol => self.handle_visual_col_mode(key),
        }
    }

    fn handle_navigation(&mut self, key: KeyEvent) {
        if let Some(pending) = self.pending_key.take() {
            if pending == 'g' && key.code == KeyCode::Char('g') {
                self.view.move_to_top();
                return;
            }
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => self.view.move_left(),
            KeyCode::Char('j') | KeyCode::Down => self.view.move_down(&self.table),
            KeyCode::Char('k') | KeyCode::Up => self.view.move_up(),
            KeyCode::Char('l') | KeyCode::Right => self.view.move_right(&self.table),
            KeyCode::Char('g') => self.pending_key = Some('g'),
            KeyCode::Char('G') => self.view.move_to_bottom(&self.table),
            KeyCode::Char('0') | KeyCode::Char('^') => self.view.move_to_first_col(),
            KeyCode::Char('$') => self.view.move_to_last_col(&self.table),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.half_page_down(&self.table);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.half_page_up();
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.page_down(&self.table);
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.page_up();
            }
            _ => {}
        }
    }

    fn handle_visual_mode(&mut self, key: KeyEvent) {
        if Self::is_escape(key) {
            self.finish_edit();
            return;
        }

        self.handle_navigation(key);

        match key.code {
            KeyCode::Char('y') => {
                if let Some(span) = self.view.yank_span(&self.table) {
                    self.clipboard.yank_span(span);
                }
                self.finish_edit();
            }
            KeyCode::Char('x') => {
                let (sr, er, sc, ec) = self.selection_bounds();
                let txn = self.create_clear_span_txn(sr, er, sc, ec);
                self.execute_and_finish(txn);
            }
            KeyCode::Char('q') => {
                let txn = self.create_drag_down_txn(false);
                self.execute_and_finish(txn);
            }
            KeyCode::Char('Q') => {
                let txn = self.create_drag_right_txn(false);
                self.execute_and_finish(txn);
            }
            _ => {}
        }
    }

    fn handle_visual_row_mode(&mut self, key: KeyEvent) {
        if Self::is_escape(key) {
            self.finish_edit();
            return;
        }

        self.handle_navigation(key);

        match key.code {
            KeyCode::Char('y') => {
                if let Some(row) = self.view.yank_row(&self.table) {
                    self.clipboard.yank_row(row);
                }
                self.finish_edit();
            }
            KeyCode::Char('x') => {
                let (sr, er, _, _) = self.selection_bounds();
                let txn = self.create_clear_span_txn(sr, er, 0, self.table.col_count() - 1);
                self.execute_and_finish(txn);
            }
            KeyCode::Char('q') => {
                let txn = self.create_drag_down_txn(true);
                self.execute_and_finish(txn);
            }
            _ => {}
        }
    }

    fn handle_visual_col_mode(&mut self, key: KeyEvent) {
        if Self::is_escape(key) {
            self.finish_edit();
            return;
        }

        self.handle_navigation(key);

        match key.code {
            KeyCode::Char('y') => {
                if let Some(col) = self.view.yank_col(&self.table) {
                    self.clipboard.yank_col(col);
                }
                self.finish_edit();
            }
            KeyCode::Char('x') => {
                let (_, _, sc, ec) = self.selection_bounds();
                let txn = self.create_clear_span_txn(0, self.table.row_count() - 1, sc, ec);
                self.execute_and_finish(txn);
            }
            KeyCode::Char('Q') => {
                let txn = self.create_drag_right_txn(true);
                self.execute_and_finish(txn);
            }
            _ => {}
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        // Handle pending key sequences
        if let Some(pending) = self.pending_key.take() {
            match (pending, key.code) {
                ('d', KeyCode::Char('r')) => {
                    if let Some(row_data) = self.table.get_row(self.view.cursor_row) {
                        let txn = Transaction::DeleteRow {
                            idx: self.view.cursor_row,
                            data: row_data.clone(),
                        };
                        self.execute(txn);
                        self.clipboard.yank_row(row_data);
                        self.view.clamp_cursor(&self.table);
                        self.view.update_col_widths(&self.table);
                        self.message = Some("Row deleted".to_string());
                    }
                    return;
                }
                ('d', KeyCode::Char('c')) => {
                    if let Some(col_data) = self.table.get_col(self.view.cursor_col) {
                        let txn = Transaction::DeleteCol {
                            idx: self.view.cursor_col,
                            data: col_data.clone(),
                        };
                        self.execute(txn);
                        self.clipboard.yank_col(col_data);
                        self.view.clamp_cursor(&self.table);
                        self.view.update_col_widths(&self.table);
                        self.message = Some("Column deleted".to_string());
                    }
                    return;
                }
                ('y', KeyCode::Char('r')) => {
                    if let Some(row) = self.table.get_row(self.view.cursor_row) {
                        self.clipboard.yank_row(row);
                        self.message = Some("Row yanked".to_string());
                    }
                    return;
                }
                ('y', KeyCode::Char('c')) => {
                    if let Some(col) = self.table.get_col(self.view.cursor_col) {
                        self.clipboard.yank_col(col);
                        self.message = Some("Column yanked".to_string());
                    }
                    return;
                }
                ('g', KeyCode::Char('g')) => {
                    self.view.move_to_top();
                    return;
                }
                _ => {}
            }
        }

        self.handle_navigation(key);

        match key.code {
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                self.edit_buffer = self.view.current_cell(&self.table).clone();
            }
            KeyCode::Char('V') => {
                self.mode = Mode::VisualRow;
                self.view.set_support();
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::VisualCol;
                self.view.set_support();
            }
            KeyCode::Char('v') => {
                self.mode = Mode::Visual;
                self.view.set_support();
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
            }
            KeyCode::Char('q') => {
                if self.dirty {
                    self.message = Some("Unsaved changes! Use :q! to force quit".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('o') => {
                let txn = Transaction::InsertRow { idx: self.view.cursor_row + 1 };
                self.execute(txn);
                self.view.cursor_row += 1;
                self.view.scroll_to_cursor();
                self.message = Some("Row added".to_string());
            }
            KeyCode::Char('O') => {
                let txn = Transaction::InsertRow { idx: self.view.cursor_row };
                self.execute(txn);
                self.view.scroll_to_cursor();
                self.message = Some("Row added".to_string());
            }
            KeyCode::Char('d') => self.pending_key = Some('d'),
            KeyCode::Char('y') => self.pending_key = Some('y'),
            KeyCode::Char('p') => {
                let (message, txn_opt) = self.clipboard.paste_as_transaction(
                    self.view.cursor_row,
                    self.view.cursor_col,
                    &self.table,
                );
                if let Some(txn) = txn_opt {
                    self.execute(txn);
                    self.view.update_col_widths(&self.table);
                }
                self.message = Some(message);
            }
            KeyCode::Char('a') => {
                let txn = Transaction::InsertCol { idx: self.view.cursor_col };
                self.execute(txn);
                self.view.update_col_widths(&self.table);
                self.message = Some("Column added".to_string());
            }
            KeyCode::Char('A') => {
                let txn = Transaction::InsertCol { idx: self.view.cursor_col + 1 };
                self.execute(txn);
                self.view.update_col_widths(&self.table);
                self.message = Some("Column added".to_string());
            }
            KeyCode::Char('X') => {
                if let Some(col_data) = self.table.get_col(self.view.cursor_col) {
                    let txn = Transaction::DeleteCol {
                        idx: self.view.cursor_col,
                        data: col_data,
                    };
                    self.execute(txn);
                    self.view.clamp_cursor(&self.table);
                    self.view.update_col_widths(&self.table);
                    self.message = Some("Column deleted".to_string());
                }
            }
            KeyCode::Char('x') => {
                let old_value = self.view.current_cell(&self.table).clone();
                let txn = Transaction::SetCell {
                    row: self.view.cursor_row,
                    col: self.view.cursor_col,
                    old_value,
                    new_value: String::new(),
                };
                self.execute(txn);
            }
            KeyCode::Char('u') => {
                if let Some(inverse) = self.history.undo() {
                    inverse.apply(&mut self.table);
                    self.view.clamp_cursor(&self.table);
                    self.view.update_col_widths(&self.table);
                    self.message = Some("Undo".to_string());
                    // Note: dirty state management for undo is complex; keeping it simple
                }
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(txn) = self.history.redo() {
                    txn.apply(&mut self.table);
                    self.view.clamp_cursor(&self.table);
                    self.view.update_col_widths(&self.table);
                    self.message = Some("Redo".to_string());
                }
            }
            _ => {}
        }
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) {
        if Self::is_escape(key) {
            let old_value = self.view.current_cell(&self.table).clone();
            let txn = Transaction::SetCell {
                row: self.view.cursor_row,
                col: self.view.cursor_col,
                old_value,
                new_value: self.edit_buffer.clone(),
            };
            self.execute_and_finish(txn);
            return;
        }

        match key.code {
            KeyCode::Backspace => { self.edit_buffer.pop(); }
            KeyCode::Char(c) => { self.edit_buffer.push(c); }
            KeyCode::Enter => {
                let old_value = self.view.current_cell(&self.table).clone();
                let txn = Transaction::SetCell {
                    row: self.view.cursor_row,
                    col: self.view.cursor_col,
                    old_value,
                    new_value: self.edit_buffer.clone(),
                };
                self.execute_and_finish(txn);
            }
            _ => {}
        }

        self.view.expand_column(self.edit_buffer.len());
    }

    fn handle_command_mode(&mut self, key: KeyEvent) {
        if Self::is_escape(key) {
            self.mode = Mode::Normal;
            self.command_buffer.clear();
            return;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(cmd) = Command::parse(&self.command_buffer) {
                    self.execute_command(cmd);
                }
                self.command_buffer.clear();
                if self.mode == Mode::Command {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Backspace => { self.command_buffer.pop(); }
            KeyCode::Char(c) => { self.command_buffer.push(c); }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::Write => {
                if let Some(ref path) = self.file_path {
                    match self.table.save_csv(path) {
                        Ok(()) => {
                            self.dirty = false;
                            self.message = Some(format!("Saved to {}", path.display()));
                        }
                        Err(e) => self.message = Some(format!("Error saving: {}", e)),
                    }
                } else {
                    self.message = Some("No file path specified".to_string());
                }
            }
            Command::Quit => {
                if self.dirty {
                    self.message = Some("Unsaved changes! Use :q! to force quit".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            Command::ForceQuit => self.should_quit = true,
            Command::WriteQuit => {
                if let Some(ref path) = self.file_path {
                    match self.table.save_csv(path) {
                        Ok(()) => self.should_quit = true,
                        Err(e) => self.message = Some(format!("Error saving: {}", e)),
                    }
                } else {
                    self.message = Some("No file path specified".to_string());
                }
            }
            Command::AddColumn => {
                let txn = Transaction::InsertCol { idx: self.view.cursor_col + 1 };
                self.execute(txn);
                self.view.update_col_widths(&self.table);
                self.message = Some("Column added".to_string());
            }
            Command::DeleteColumn => {
                if let Some(col_data) = self.table.get_col(self.view.cursor_col) {
                    let txn = Transaction::DeleteCol {
                        idx: self.view.cursor_col,
                        data: col_data,
                    };
                    self.execute(txn);
                    self.view.clamp_cursor(&self.table);
                    self.view.update_col_widths(&self.table);
                    self.message = Some("Column deleted".to_string());
                }
            }
            Command::ToggleHeader => {
                self.header_mode = !self.header_mode;
                self.message = Some(format!(
                    "Header mode {}",
                    if self.header_mode { "on" } else { "off" }
                ));
            }
            Command::Calc => {
                let calc = Calculator::new(&self.table);
                match calc.evaluate_all() {
                    Ok(updates) => {
                        if updates.is_empty() {
                            self.message = Some("No formulas found".to_string());
                            return;
                        }
                        let txns: Vec<Transaction> = updates
                            .into_iter()
                            .map(|(row, col, new_value)| {
                                let old_value = self.table.get_cell(row, col)
                                    .cloned()
                                    .unwrap_or_default();
                                Transaction::SetCell { row, col, old_value, new_value }
                            })
                            .collect();
                        let count = txns.len();
                        self.execute(Transaction::Batch(txns));
                        self.view.update_col_widths(&self.table);
                        self.message = Some(format!("Evaluated {} formula(s)", count));
                    }
                    Err(e) => self.message = Some(format!("{}", e)),
                }
            }
            Command::NavigateRow(row) => self.view.cursor_row = row,
            Command::NavigateCell(cell) => {
                self.view.cursor_row = cell.row;
                self.view.cursor_col = cell.col;
            }
            Command::Unknown(s) => self.message = Some(format!("Unknown command: {}", s)),
        }
    }
}
