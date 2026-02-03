use std::io;
use std::cmp;
use std::time::Duration;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::rc::Rc;
use std::cell::RefCell;

use crossterm::event::{self, poll, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::calculator::Calculator;
use crate::canvas::Canvas;
use crate::clipboard::{Clipboard, RegisterContent};
use crate::command::{Command, ReplaceCommand};
use crate::input::{
    is_escape, CommandHandler, InsertHandler, KeyBuffer, KeyBufferResult, KeyResult,
    NavigationHandler, SearchHandler, SequenceAction, VisualHandler
};
use crate::mode::Mode;
use crate::operations;
use crate::plugin::{PluginManager, PluginAction, CommandContext};
use crate::table::{SortDirection, Table};
use crate::tableview::TableView;
use crate::transaction::{History, Transaction};
use crate::ui;
use crate::fileio::FileIO;
use crate::style::Style;
use crate::progress::Progress;
use crate::rowmanager::{FilterType, RowManager};
use crate::util::ColumnType;
use crate::visual::{SelectionInfo, VisualType};

/// Result from a background operation
pub enum BackgroundResult {
    SortComplete {
        permutation: Vec<usize>,
        direction: SortDirection,
        sort_type: ColumnType,
        is_column_sort: bool,
    },
}

/// Pending operation to be executed after the next render
/// This allows progress to be displayed before the operation runs
pub enum PendingOp {
    Undo,
    Redo,
    Calc { formula_count: usize },
}

pub struct App {
    pub table: Table,
    pub view: TableView,
    pub clipboard: Clipboard,
    pub history: History,
    pub style: Style,
    pub mode: Mode,
    pub file_io: FileIO,
    pub row_manager: Rc<RefCell<RowManager>>,
    pub dirty: bool,
    pub calling_mode: Option<Mode>,
    pub message: Option<String>,
    pub should_quit: bool,
    pub header_mode: bool,
    pub precision: Option<usize>,  // Display precision for numbers (None = auto)
    pub progress: Option<(String, Progress)>,  // Optional progress indicator (operation name, progress)
    pub canvas: Canvas,  // Canvas overlay for displaying text/images
    pending_op: Option<PendingOp>,  // Pending operation to execute after next render
    // Background task handling
    bg_receiver: Option<Receiver<BackgroundResult>>,
    #[allow(dead_code)]
    bg_handle: Option<JoinHandle<()>>,
    // Mode handlers
    key_buffer: KeyBuffer,
    nav_handler: NavigationHandler,
    search_handler: SearchHandler,
    insert_handler: InsertHandler,
    command_handler: CommandHandler,
    // Plugin system
    plugin_manager: PluginManager,
}

impl App {
    pub fn new(table: Table, file_io: FileIO) -> Self {
        let row_manager = Rc::new(RefCell::new(RowManager::new()));
        let view = TableView::new(row_manager.clone());
        let clipboard = Clipboard::new();

        let mut plugin_manager = PluginManager::new();
        let _ = plugin_manager.load_plugins();

        Self {
            table,
            view,
            clipboard,
            history: History::new(),
            style: Style::new(),
            mode: Mode::Normal,
            file_io,
            row_manager,
            dirty: false,
            calling_mode: None,
            message: None,
            should_quit: false,
            header_mode: true,
            precision: None,
            progress: None,
            canvas: Canvas::new(),
            pending_op: None,
            bg_receiver: None,
            bg_handle: None,
            key_buffer: KeyBuffer::new(),
            nav_handler: NavigationHandler::new(),
            search_handler: SearchHandler::new(),
            insert_handler: InsertHandler::new(),
            command_handler: CommandHandler::new(),
            plugin_manager,
        }
    }

    // Accessor methods for UI
    pub fn search_pattern(&self) -> Option<&String> {
        self.search_handler.pattern.as_ref()
    }

    pub fn edit_buffer(&self) -> &str {
        &self.insert_handler.buffer
    }

    pub fn edit_cursor(&self) -> usize {
        self.insert_handler.cursor
    }

    pub fn command_buffer(&self) -> &str {
        &self.command_handler.buffer
    }

    pub fn search_buffer(&self) -> &str {
        &self.search_handler.buffer
    }

    pub fn key_buffer_display(&self) -> String {
        self.key_buffer.display()
    }

    /// Start a progress indicator for a long-running operation
    pub fn start_progress(&mut self, operation: &str, total: usize) -> Progress {
        let progress = Progress::new(total);
        self.progress = Some((operation.to_string(), progress.clone()));
        progress
    }

    /// Clear the progress indicator
    pub fn clear_progress(&mut self) {
        self.progress = None;
    }

    /// Execute a pending operation (called after render so progress is visible)
    fn execute_pending_op(&mut self, op: PendingOp) {
        match op {
            PendingOp::Undo => {
                if let Some(inverse) = self.history.undo() {
                    // Handle filter state if this is a SetFilter transaction
                    if let Some(filter_state) = inverse.filter_state() {
                        self.row_manager.borrow_mut().restore(filter_state.clone());
                        self.view.move_to_top();
                    }
                    inverse.apply(&mut self.table);
                    self.view.clamp_cursor(&self.table);
                    self.message = Some("Undo".to_string());
                }
                self.clear_progress();
            }
            PendingOp::Redo => {
                if let Some(txn) = self.history.redo() {
                    // Handle filter state if this is a SetFilter transaction
                    if let Some(filter_state) = txn.filter_state() {
                        self.row_manager.borrow_mut().restore(filter_state.clone());
                        self.view.move_to_top();
                    }
                    txn.apply(&mut self.table);
                    self.view.clamp_cursor(&self.table);
                    self.message = Some("Redo".to_string());
                }
                self.clear_progress();
            }
            PendingOp::Calc { formula_count } => {
                let calc = Calculator::new(&self.table, self.header_mode);
                match calc.evaluate_all() {
                    Ok(updates) => {
                        if updates.is_empty() {
                            self.message = Some("No formulas found".to_string());
                        } else {
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
                            self.message = Some(format!("Evaluated {} formula(s)", count));
                        }
                    }
                    Err(e) => self.message = Some(format!("{}", e)),
                }
                self.clear_progress();
                let _ = formula_count; // Suppress unused warning
            }
        }
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        while !self.should_quit {
            // Check for completed background operations
            self.poll_background_result();

            terminal.draw(|f| ui::render(f, self, self.row_manager.clone()))?;

            // Execute pending operation after render (so progress bar is visible)
            if let Some(op) = self.pending_op.take() {
                self.execute_pending_op(op);
                continue; // Re-render immediately after
            }

            if poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    self.message = None;
                    self.handle_key(key);
                }
            }
        }
        Ok(())
    }

    /// Check for and handle completed background operations
    fn poll_background_result(&mut self) {
        if let Some(ref receiver) = self.bg_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.handle_background_result(result);
                    self.bg_receiver = None;
                    self.bg_handle = None;
                    self.clear_progress();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still working, progress is updated by the background thread
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Thread died unexpectedly
                    self.bg_receiver = None;
                    self.bg_handle = None;
                    self.clear_progress();
                    self.message = Some("Operation failed".to_string());
                }
            }
        }
    }

    /// Handle a completed background operation
    fn handle_background_result(&mut self, result: BackgroundResult) {
        match result {
            BackgroundResult::SortComplete { permutation, direction, sort_type, is_column_sort } => {
                // Empty permutation means already sorted
                if permutation.is_empty() {
                    self.message = Some("Already sorted".to_string());
                    return;
                }

                if is_column_sort {
                    self.table.apply_col_permutation(&permutation);
                    let txn = Transaction::PermuteCols { permutation };
                    self.history.record(txn);
                } else {
                    self.table.apply_row_permutation(&permutation);
                    let txn = Transaction::PermuteRows { permutation };
                    self.history.record(txn);
                }
                self.dirty = true;

                let type_str = match sort_type {
                    ColumnType::Numeric => "numeric",
                    ColumnType::Text => "text",
                };
                let dir_str = match direction {
                    SortDirection::Ascending => "ascending",
                    SortDirection::Descending => "descending",
                };
                if is_column_sort {
                    self.message = Some(format!("Columns sorted {} ({})", dir_str, type_str));
                } else {
                    self.message = Some(format!("Sorted {} ({})", dir_str, type_str));
                }
            }
        }
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
    }

    // === Key handling ===

    fn handle_key(&mut self, key: KeyEvent) {
        // If canvas is visible, handle canvas-specific keys first
        if self.canvas.visible {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.canvas.hide();
                    return;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.canvas.scroll_down(1);
                    return;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.canvas.scroll_up(1);
                    return;
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.canvas.scroll_down(10);
                    return;
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.canvas.scroll_up(10);
                    return;
                }
                _ => {
                    // Ignore other keys when canvas is visible
                    return;
                }
            }
        }

        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Command => self.handle_command_mode(key),
            Mode::Visual => self.handle_visual_mode(key, VisualType::Cell),
            Mode::VisualRow => self.handle_visual_mode(key, VisualType::Row),
            Mode::VisualCol => self.handle_visual_mode(key, VisualType::Col),
            Mode::Search => self.handle_search_mode(key),
        }
    }

    fn handle_visual_mode(&mut self, key: KeyEvent, visual_type: VisualType) {
        let handler = VisualHandler::new(visual_type);
        let result = handler.handle_key(
            key,
            &mut self.view,
            &self.table,
            &mut self.clipboard,
            &self.nav_handler,
            &mut self.key_buffer,
        );
        self.process_key_result(result);
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        // Process through key buffer for sequences
        match self.key_buffer.process(key) {
            KeyBufferResult::Action(action, count) => {
                self.execute_sequence_action(action, count);
                return;
            }
            KeyBufferResult::Pending => {
                // Waiting for more keys
                return;
            }
            KeyBufferResult::Fallthrough(key, count) => {
                // Process as single key
                self.handle_normal_key(key, count);
            }
        }
    }

    fn execute_sequence_action(&mut self, action: SequenceAction, count: usize) {
        match action {
            SequenceAction::SelectRegister(reg) => {
                if let Err(e) = self.clipboard.select_register(reg) {
                    self.message = Some(e);
                }
                // Don't clear key buffer - next action will use this register
            }
            SequenceAction::DeleteRow => {
                let start_row = self.view.cursor_row;
                let actual_count = count.min(self.table.row_count().saturating_sub(start_row));

                if actual_count == 0 {
                    return;
                }

                if self.row_manager.borrow().is_filtered {
                    self.message = Some("Delete is forbidden in filtered views.".to_string());
                    return;
                }
                let rows = self.table.get_rows_cloned(start_row, actual_count);
                if !rows.is_empty() {
                    self.clipboard.store_deleted(RegisterContent::from_rows(rows.clone()));

                    let txn = Transaction::DeleteRowsBulk {
                        idx: start_row,
                        data: rows,
                    };
                    self.execute(txn);
                }
                self.view.clamp_cursor(&self.table);
                let msg = if actual_count == 1 { "Row deleted".to_string() } else { format!("{} rows deleted", actual_count) };
                self.message = Some(msg);
            }
            SequenceAction::DeleteCol => {
                let start_col = self.view.cursor_col;
                let end_col = (start_col + count).min(self.table.col_count());
                let actual_count = end_col - start_col;

                // Store deleted columns (doesn't affect yank register)
                let cols: Vec<Vec<String>> = (0..self.table.row_count())
                    .map(|r| {
                        (start_col..end_col)
                            .map(|c| self.table.get_cell(r, c).cloned().unwrap_or_default())
                            .collect()
                    })
                    .collect();
                if !cols.is_empty() {
                    self.clipboard.store_deleted(RegisterContent::from_cols(cols));
                }

                // Delete columns (always delete at start_col since indices shift)
                for _ in 0..actual_count {
                    if let Some(col_data) = self.table.get_col_cloned(start_col) {
                        let txn = Transaction::DeleteCol {
                            idx: start_col,
                            data: col_data,
                        };
                        self.execute(txn);
                    }
                }
                self.view.clamp_cursor(&self.table);
                let msg = if actual_count == 1 { "Column deleted".to_string() } else { format!("{} columns deleted", actual_count) };
                self.message = Some(msg);
            }
            SequenceAction::YankRow => {
                let start_row = self.view.cursor_row;
                let actual_count = count.min(self.table.row_count().saturating_sub(start_row));

                // Use bulk get for efficiency, if we can
                let rows = if self.row_manager.borrow().is_filtered {
                    let it = (start_row..self.table.row_count()).filter(|&i| self.row_manager.borrow().is_row_live(i)).take(actual_count);
                    it.filter_map(|i| self.table.get_row_cloned(i)).collect()
                } else {
                    self.table.get_rows_cloned(start_row, actual_count)
                };
                if !rows.is_empty() {
                    self.clipboard.yank_rows(rows);
                }
                let msg = if actual_count == 1 { "Row yanked".to_string() } else { format!("{} rows yanked", actual_count) };
                self.message = Some(msg);
            }
            SequenceAction::YankCol => {
                let start_col = self.view.cursor_col;
                let end_col = (start_col + count).min(self.table.col_count());
                let actual_count = end_col - start_col;

                let cols: Vec<Vec<String>> = (0..self.table.row_count())
                    .map(|r| {
                        (start_col..end_col)
                            .map(|c| self.table.get_cell(r, c).cloned().unwrap_or_default())
                            .collect()
                    })
                    .collect();
                if !cols.is_empty() {
                    self.clipboard.yank_cols(cols);
                }
                let msg = if actual_count == 1 { "Column yanked".to_string() } else { format!("{} columns yanked", actual_count) };
                self.message = Some(msg);
            }
            SequenceAction::Yank => {
                if let Some(row) = self.table.get_row_cloned(self.view.cursor_row) {
                    self.clipboard.yank_span(vec![vec![row[self.view.cursor_col].clone()]]);
                    self.message = Some("Row yanked".to_string());
                }
            }
            SequenceAction::Delete => {
                if self.row_manager.borrow().is_filtered {
                    self.message = Some("Adding rows is forbidden in filtered views.".to_string());
                    return;
                }
                if let Some(row_data) = self.table.get_row_cloned(self.view.cursor_row) {
                    self.clipboard.store_deleted(RegisterContent::from_rows(vec![row_data.clone()]));
                    let txn = Transaction::DeleteRow {
                        idx: self.view.cursor_row,
                        data: row_data,
                    };
                    self.execute(txn);
                    self.view.clamp_cursor(&self.table);
                    self.message = Some("Row deleted".to_string());
                }
            }
             SequenceAction::MoveToTop
             | SequenceAction::MoveDown
             | SequenceAction::MoveUp
             | SequenceAction::MoveLeft
             | SequenceAction::MoveRight => {
                self.nav_handler.handle_sequence(action, count, &mut self.view, &self.table);
            }

            // Format actions are only meaningful in visual mode
            SequenceAction::FormatDefault
            | SequenceAction::FormatCommas
            | SequenceAction::FormatCurrency
            | SequenceAction::FormatScientific
            | SequenceAction::FormatPercentage => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent, _count: usize) {
        // Handle navigation (hjkl already handled by KeyBuffer with count)
        self.nav_handler.handle(key, &mut self.view, &self.table);

        match key.code {
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                let current = operations::current_cell(&self.view, &self.table).clone();
                self.insert_handler.start_edit(current);
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
                self.calling_mode = Some(self.mode);
                self.mode = Mode::Command;
                self.command_handler.start();
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
                if self.row_manager.borrow().is_filtered {
                    self.message = Some("Adding rows is forbidden in filtered views.".to_string());
                    return;
                }
                let txn = Transaction::InsertRow { idx: self.view.cursor_row + 1 };
                self.execute(txn);
                self.view.cursor_row += 1;
                self.view.scroll_to_cursor();
                self.message = Some("Row added".to_string());
            }
            KeyCode::Char('O') => {
                if self.row_manager.borrow().is_filtered {
                    self.message = Some("Adding rows is forbidden in filtered views.".to_string());
                    return;
                }
                let txn = Transaction::InsertRow { idx: self.view.cursor_row };
                self.execute(txn);
                self.view.scroll_to_cursor();
                self.message = Some("Row added".to_string());
            }
            KeyCode::Char('p') => {
                let (message, txn_opt) = self.clipboard.paste_as_transaction(
                    self.view.cursor_row,
                    self.view.cursor_col,
                    &self.table,
                );
                if let Some(txn) = txn_opt {
                    self.execute(txn);
                            }
                self.message = Some(message);
            }
            KeyCode::Char('a') => {
                let txn = Transaction::InsertCol { idx: self.view.cursor_col };
                self.execute(txn);
                        self.message = Some("Column added".to_string());
            }
            KeyCode::Char('A') => {
                let txn = Transaction::InsertCol { idx: self.view.cursor_col + 1 };
                self.execute(txn);
                        self.message = Some("Column added".to_string());
            }
            KeyCode::Char('X') => {
                if let Some(col_data) = self.table.get_col_cloned(self.view.cursor_col) {
                    let txn = Transaction::DeleteCol {
                        idx: self.view.cursor_col,
                        data: col_data,
                    };
                    self.execute(txn);
                    self.view.clamp_cursor(&self.table);
                                self.message = Some("Column deleted".to_string());
                }
            }
            KeyCode::Char('x') => {
                let old_value = operations::current_cell(&self.view, &self.table).clone();
                let txn = Transaction::SetCell {
                    row: self.view.cursor_row,
                    col: self.view.cursor_col,
                    old_value,
                    new_value: String::new(),
                };
                self.execute(txn);
            }
            KeyCode::Char('u') => {
                // Check if undo is large before executing
                if let Some(txn) = self.history.peek_undo() {
                    if txn.is_large() {
                        let size = txn.estimated_size();
                        self.start_progress("Undoing", size);
                        self.pending_op = Some(PendingOp::Undo);
                    } else if let Some(inverse) = self.history.undo() {
                        // Handle filter state if this is a SetFilter transaction
                        if let Some(filter_state) = inverse.filter_state() {
                            self.row_manager.borrow_mut().restore(filter_state.clone());
                            self.view.move_to_top();
                        }
                        inverse.apply(&mut self.table);
                        self.view.clamp_cursor(&self.table);
                        self.message = Some("Undo".to_string());
                    }
                }
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Check if redo is large before executing
                if let Some(txn) = self.history.peek_redo() {
                    if txn.is_large() {
                        let size = txn.estimated_size();
                        self.start_progress("Redoing", size);
                        self.pending_op = Some(PendingOp::Redo);
                    } else if let Some(txn) = self.history.redo() {
                        // Handle filter state if this is a SetFilter transaction
                        if let Some(filter_state) = txn.filter_state() {
                            self.row_manager.borrow_mut().restore(filter_state.clone());
                            self.view.move_to_top();
                        }
                        txn.apply(&mut self.table);
                        self.view.clamp_cursor(&self.table);
                        self.message = Some("Redo".to_string());
                    }
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.search_handler.start_search();
            }
            KeyCode::Char('n') => {
                if let Some(msg) = self.search_handler.goto_next(&mut self.view) {
                    self.message = Some(msg);
                }
            }
            KeyCode::Char('N') => {
                if let Some(msg) = self.search_handler.goto_prev(&mut self.view) {
                    self.message = Some(msg);
                }
            }
            _ => {}
        }
    }

    fn handle_search_mode(&mut self, key: KeyEvent) {
        let result = self.search_handler.handle_key(key);
        match result {
            KeyResult::Finish => {
                if self.search_handler.pattern.is_some() {
                    if let Some(msg) = self.search_handler.perform_search(&self.table) {
                        self.message = Some(msg);
                    }
                    if let Some(msg) = self.search_handler.goto_next(&mut self.view) {
                        self.message = Some(msg);
                    }
                }
                self.mode = Mode::Normal;
            }
            KeyResult::Continue => {}
            _ => {}
        }
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) {
        if is_escape(key) || key.code == KeyCode::Enter {
            let old_value = operations::current_cell(&self.view, &self.table).clone();
            let txn = Transaction::SetCell {
                row: self.view.cursor_row,
                col: self.view.cursor_col,
                old_value,
                new_value: self.insert_handler.buffer.clone(),
            };
            self.execute_and_finish(txn);
            self.table.recompute_col_widths();
            return;
        }

        self.insert_handler.handle_key(key, &self.view);
        self.table.expand_col_width(self.view.cursor_col, self.insert_handler.buffer.len());
    }

    fn handle_command_mode(&mut self, key: KeyEvent) {
        if is_escape(key) {
            self.mode = Mode::Normal;
            self.calling_mode = None;
            self.command_handler.buffer.clear();
            return;
        }

        if let Some(cmd_str) = self.command_handler.handle_key(key) {
            if let Some(cmd) = Command::parse(&cmd_str) {
                self.execute_command(cmd);
            }
            if self.mode == Mode::Command {
                self.mode = Mode::Normal;
            }
        }
    }

    fn process_key_result(&mut self, result: KeyResult) {
        match result {
            KeyResult::Continue => {}
            KeyResult::Finish => {
                self.finish_edit();
            }
            KeyResult::SwitchMode(mode) => {
                let prev_mode = Some(self.mode);
                self.mode = mode;
                if mode == Mode::Command {
                    self.calling_mode = prev_mode;
                    self.command_handler.start();
                }
            }
            KeyResult::Execute(txn) => {
                self.execute(txn);
            }
            KeyResult::ExecuteAndFinish(txn) => {
                self.execute_and_finish(txn);
            }
            KeyResult::Message(msg) => {
                self.message = Some(msg);
            }
            KeyResult::Quit => {
                if self.dirty {
                    self.message = Some("Unsaved changes! Use :q! to force quit".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            KeyResult::ForceQuit => {
                self.should_quit = true;
            }
        }
    }

    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::Write => {
                match self.file_io.write(&mut self.table) {
                    Ok(()) => {
                        self.dirty = false;
                        self.message = Some(format!("Saved to {}", self.file_io.file_name()));
                    }
                    Err(e) => self.message = Some(format!("Error saving: {}", e)),
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
                match self.file_io.write(&mut self.table) {
                    Ok(()) => {
                        self.dirty = false;
                        self.message = Some(format!("Saved to {}", self.file_io.file_name()));
                    }
                    Err(e) => self.message = Some(format!("Error saving: {}", e)),
                }
                self.should_quit = true;
            }
            Command::AddColumn => {
                let txn = Transaction::InsertCol { idx: self.view.cursor_col + 1 };
                self.execute(txn);
                        self.message = Some("Column added".to_string());
            }
            Command::DeleteColumn => {
                if let Some(col_data) = self.table.get_col_cloned(self.view.cursor_col) {
                    let txn = Transaction::DeleteCol {
                        idx: self.view.cursor_col,
                        data: col_data,
                    };
                    self.execute(txn);
                    self.view.clamp_cursor(&self.table);
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
                // For large tables, queue as pending op to show progress
                let cell_count = self.table.row_count() * self.table.col_count();
                if cell_count >= 50_000 {
                    self.start_progress("Calculating", cell_count);
                    self.pending_op = Some(PendingOp::Calc { formula_count: cell_count });
                } else {
                    let calc = Calculator::new(&self.table, self.header_mode);
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
                            self.message = Some(format!("Evaluated {} formula(s)", count));
                        }
                        Err(e) => self.message = Some(format!("{}", e)),
                    }
                }
            }
            Command::Grid => self.style.toggle_grid(),
            Command::NavigateRow(row) => self.view.cursor_row = row,
            Command::NavigateCell(cell) => {
                self.view.cursor_row = cell.row;
                self.view.cursor_col = cell.col;
            }
            Command::Sort => self.sort_by_column(SortDirection::Ascending),
            Command::SortDesc => self.sort_by_column(SortDirection::Descending),
            Command::SortRow => self.sort_by_row(SortDirection::Ascending),
            Command::SortRowDesc => self.sort_by_row(SortDirection::Descending),
            Command::Replace(ref replace_cmd) => {
                self.execute_replace(replace_cmd.clone());
            }
            Command::Theme(name) => {
                use crate::style::Theme;
                if let Some(theme) = Theme::by_name(&name) {
                    self.style.set_theme(theme);
                    self.message = Some(format!("Theme set to '{}'", name));
                } else {
                    self.message = Some(format!(
                        "Unknown theme '{}'. Available: {}",
                        name,
                        Theme::builtin_names().join(", ")
                    ));
                }
            }
            Command::ThemeList => {
                use crate::style::Theme;
                self.message = Some(format!(
                    "Available themes: {}",
                    Theme::builtin_names().join(", ")
                ));
            }
            Command::Clip => {
                match self.clipboard.to_system() {
                    Ok(msg) => self.message = Some(msg),
                    Err(e) => self.message = Some(e),
                }
            }
            Command::Fork => {
                self.file_io = self.file_io.fork();

                let fname = self.file_io.file_name();

                self.message = Some(format!("File forked successfully, you are now editing: {}", fname));
            }
            Command::SysPaste => {
                match self.clipboard.from_system() {
                    Ok(msg) => self.message = Some(msg),
                    Err(e) => self.message = Some(e),
                }
            }
            Command::PluginList => {
                let commands = self.plugin_manager.list_commands();
                if commands.is_empty() {
                    self.message = Some(format!(
                        "No plugins loaded. Add .lua files to {}",
                        crate::plugin::plugin_dir().display()
                    ));
                } else {
                    self.message = Some(format!("Plugins: {}", commands.into_iter().cloned().collect::<Vec<_>>().join(", ")));
                }
            }
            Command::Precision(prec) => {
                self.precision = prec;
                let msg = match prec {
                    Some(n) => format!("Display precision set to {} decimal places", n),
                    None => "Display precision set to auto".to_string(),
                };
                self.message = Some(msg);
            }
            Command::Custom { name, args } => {
                self.execute_plugin(&name, &args);
            }
            Command::Unknown(s) => {
                // Check if it's a plugin command
                let parts: Vec<&str> = s.split_whitespace().collect();
                if let Some(cmd_name) = parts.first() {
                    if self.plugin_manager.has_command(cmd_name) {
                        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                        self.execute_plugin(cmd_name, &args);
                        return;
                    }
                }
                self.message = Some(format!("Unknown command: {}", s));
            }
            Command::Filter(filter_type) => {
                // Capture old state for undo
                let old_state = self.row_manager.borrow().snapshot();

                // This is necessary so the view doesn't go out of sync
                self.view.move_to_top();
                if filter_type == FilterType::Default {
                    self.row_manager.borrow_mut().remove_filter();
                    self.message = Some("Filter removed".to_string());
                } else if let FilterType::PredicateFilter(pred) = filter_type {
                    let active_col = self.view.cursor_col;
                    let column_type = self.table.probe_column_type(active_col, self.header_mode);
                    self.row_manager.borrow_mut().predicate_filter(&self.table, active_col, pred, column_type, self.header_mode);
                    self.message = Some("Filter applied".to_string());
                } else {
                    self.message = Some("Filter not recognized".to_string());
                }

                // Capture new state and record transaction
                let new_state = self.row_manager.borrow().snapshot();
                let txn = Transaction::SetFilter { old_state, new_state };
                self.history.record(txn);
            }
            Command::Canvas => {
                // Debug command: show canvas with sample content
                self.canvas.clear();
                self.canvas.set_title("Debug Canvas");
                self.canvas.add_header("Canvas Overlay Demo");
                self.canvas.add_separator();
                self.canvas.add_text(format!("Table: {} rows x {} cols", self.table.row_count(), self.table.col_count()));
                self.canvas.add_text(format!("Cursor: row {}, col {}", self.view.cursor_row + 1, self.view.cursor_col + 1));
                self.canvas.add_text(format!("Header mode: {}", if self.header_mode { "on" } else { "off" }));
                self.canvas.add_blank();
                self.canvas.add_header("Sample ASCII Art");
                self.canvas.add_box(20, 5, ' ');
                self.canvas.add_blank();
                self.canvas.add_styled_text(
                    "This is styled text (cyan)",
                    Some(ratatui::style::Color::Cyan),
                    None,
                    false
                );
                self.canvas.add_styled_text(
                    "This is bold text",
                    None,
                    None,
                    true
                );
                self.canvas.show();
                self.message = Some("Canvas opened (q/Esc to close, j/k to scroll)".to_string());
            }
        }
        self.calling_mode = None;
    }

    fn get_selection_info(&self) -> SelectionInfo {
        SelectionInfo {
            mode: self.mode,
            start_row: cmp::min(self.view.support_row, self.view.cursor_row),
            end_row: cmp::max(self.view.support_row, self.view.cursor_row),
            start_col: cmp::min(self.view.support_col, self.view.cursor_col),
            end_col: cmp::max(self.view.support_col, self.view.cursor_col),
        }
    }

    fn execute_plugin(&mut self, name: &str, args: &[String]) {
        let ctx = CommandContext {
            cursor_row: self.view.cursor_row,
            cursor_col: self.view.cursor_col,
            row_count: self.table.row_count(),
            col_count: self.table.col_count(),
            selection: self.get_selection_info()
        };

        // Create a closure to get cell values
        let table = &self.table;
        let get_cell = |row: usize, col: usize| -> Option<String> {
            table.get_cell(row, col).cloned()
        };

        match self.plugin_manager.execute(name, args, &ctx, get_cell) {
            Ok(result) => {
                // Process actions from the plugin
                let mut txns = Vec::new();
                for action in result.actions {
                    match action {
                        PluginAction::SetCell { row, col, value } => {
                            if let Some(old_value) = self.table.get_cell(row, col).cloned() {
                                txns.push(Transaction::SetCell {
                                    row,
                                    col,
                                    old_value,
                                    new_value: value,
                                });
                            }
                        }
                        PluginAction::InsertRow { at } => {
                            txns.push(Transaction::InsertRow { idx: at });
                        }
                        PluginAction::DeleteRow { at } => {
                            let data = self.table.get_row_cloned(at).unwrap_or_default();
                            txns.push(Transaction::DeleteRow { idx: at, data });
                        }
                        PluginAction::InsertCol { at } => {
                            txns.push(Transaction::InsertCol { idx: at });
                        }
                        PluginAction::DeleteCol { at } => {
                            let data = self.table.get_col_cloned(at).unwrap_or_default();
                            txns.push(Transaction::DeleteCol { idx: at, data });
                        }
                        // Canvas actions (not recorded in transactions)
                        PluginAction::CanvasClear => {
                            self.canvas.clear();
                        }
                        PluginAction::CanvasShow => {
                            self.canvas.show();
                        }
                        PluginAction::CanvasHide => {
                            self.canvas.hide();
                        }
                        PluginAction::CanvasSetTitle { title } => {
                            self.canvas.set_title(title);
                        }
                        PluginAction::CanvasAddText { text } => {
                            self.canvas.add_text(text);
                        }
                        PluginAction::CanvasAddHeader { text } => {
                            self.canvas.add_header(text);
                        }
                        PluginAction::CanvasAddSeparator => {
                            self.canvas.add_separator();
                        }
                        PluginAction::CanvasAddBlank => {
                            self.canvas.add_blank();
                        }
                        PluginAction::CanvasAddStyledText { text, fg, bg, bold } => {
                            self.canvas.add_styled_text(
                                text,
                                fg.map(|c| c.to_ratatui()),
                                bg.map(|c| c.to_ratatui()),
                                bold
                            );
                        }
                        PluginAction::CanvasAddImage { rows, title } => {
                            self.canvas.add_image(rows, title);
                        }
                        PluginAction::PromptRequest { question, default } => {

                        }
                    }
                }

                if !txns.is_empty() {
                    self.execute(Transaction::Batch(txns));
                }

                if let Some(msg) = result.message {
                    self.message = Some(msg);
                }
            }
            Err(e) => {
                self.message = Some(format!("Plugin error: {}", e));
            }
        }
    }

    fn execute_replace(&mut self, cmd: ReplaceCommand) {
        use crate::command::ReplaceScope;

        // Determine which cells to search
        let (row_range, col_range) = match cmd.scope {
            ReplaceScope::All => {
                (0..self.table.row_count(), 0..self.table.col_count())
            }
            ReplaceScope::Selection => {
                if self.calling_mode.map_or(false, |x| x.is_visual()) {
                    // Use the visual selection bounds (stored in view)
                    let (start_row, end_row) = if self.calling_mode != Some(Mode::VisualCol) {
                        (std::cmp::min(self.view.cursor_row, self.view.support_row),
                            std::cmp::max(self.view.cursor_row, self.view.support_row))
                    } else {
                        (0, self.table.row_count()-1)
                    };
                    let (start_col, end_col) = if self.calling_mode != Some(Mode::VisualRow) {
                        (std::cmp::min(self.view.cursor_col, self.view.support_col),
                            std::cmp::max(self.view.cursor_col, self.view.support_col))
                    } else {
                        (0, self.table.col_count()-1)
                    };
                    (start_row..end_row + 1, start_col..end_col + 1)
                } else {
                    (self.view.cursor_row..self.view.cursor_row+1, self.view.cursor_col..self.view.cursor_col+1)
                }
            }
        };

        let mut replacements = 0;
        let mut txns: Vec<Transaction> = Vec::new();

        let mut found = false;

        for row in row_range.clone() {
            for col in col_range.clone() {
                if let Some(cell) = self.table.get_cell(row, col) {
                    found = true;

                    let old_value = cell.clone();
                    let new_value = if cmd.global {
                        old_value.replace(&cmd.pattern, &cmd.replacement)
                    } else {
                        old_value.replacen(&cmd.pattern, &cmd.replacement, 1)
                    };

                    if new_value != old_value {
                        replacements += 1;
                        txns.push(Transaction::SetCell {
                            row,
                            col,
                            old_value,
                            new_value,
                        });
                    }
                }

                if found && !cmd.global {
                    break;
                }
            }
            if found && !cmd.global {
                break;
            }
        }

        if txns.is_empty() {
            self.message = Some(format!("Pattern not found: {}", cmd.pattern));
        } else {
            self.execute(Transaction::Batch(txns));
                self.message = Some(format!("{} replacement(s) made", replacements));
        }
    }

    fn sort_by_column(&mut self, direction: SortDirection) {
        // Don't start a new sort if one is already running
        if self.bg_receiver.is_some() {
            self.message = Some("Sort already in progress".to_string());
            return;
        }

        let sort_col = self.view.cursor_col;
        let skip_header = self.header_mode;
        let row_count = self.table.row_count();

        // For small tables, sort synchronously
        if row_count < 50_000 {
            self.sort_by_column_sync(direction);
            return;
        }

        // Extract the column data for sorting (clone to send to thread)
        let sort_type = self.table.probe_column_type(sort_col, skip_header);
        let col_data: Vec<String> = (0..row_count)
            .map(|row| {
                self.table.get_cell(row, sort_col)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();

        // Set up progress tracking
        let progress = self.start_progress("Sorting", row_count);

        // Create channel for result
        let (tx, rx) = mpsc::channel();
        self.bg_receiver = Some(rx);

        // Spawn background thread
        let handle = thread::spawn(move || {
            let start_row = if skip_header { 1 } else { 0 };

            // Build keyed vector with progress updates
            let mut keyed: Vec<(usize, SortKey)> = Vec::with_capacity(row_count - start_row);

            for (i, row) in (start_row..row_count).enumerate() {
                let key = match sort_type {
                    ColumnType::Numeric => {
                        let val = crate::format::parse_numeric(col_data[row].trim())
                            .unwrap_or(f64::NAN);
                        SortKey::Numeric(val)
                    }
                    ColumnType::Text => {
                        SortKey::Text(col_data[row].to_lowercase())
                    }
                };
                keyed.push((row, key));

                // Update progress periodically
                if i % 10000 == 0 {
                    progress.set(i);
                }
            }

            progress.set(row_count / 2); // Halfway point before sort

            // Sort
            keyed.sort_unstable_by(|(idx_a, key_a), (idx_b, key_b)| {
                let cmp = key_a.cmp(key_b);
                match direction {
                    SortDirection::Ascending => cmp.then(idx_a.cmp(idx_b)),
                    SortDirection::Descending => cmp.reverse().then(idx_a.cmp(idx_b)),
                }
            });

            progress.set(row_count);

            // Build permutation
            let mut permutation: Vec<usize> = if skip_header {
                vec![0]
            } else {
                Vec::new()
            };
            permutation.extend(keyed.into_iter().map(|(row, _)| row));

            // Check if already sorted
            let already_sorted = permutation.iter().enumerate().all(|(i, &idx)| i == idx);
            if already_sorted {
                let _ = tx.send(BackgroundResult::SortComplete {
                    permutation: Vec::new(), // Empty signals already sorted
                    direction,
                    sort_type,
                    is_column_sort: false,
                });
            } else {
                let _ = tx.send(BackgroundResult::SortComplete {
                    permutation,
                    direction,
                    sort_type,
                    is_column_sort: false,
                });
            }
        });

        self.bg_handle = Some(handle);
    }

    /// Synchronous sort for small tables
    fn sort_by_column_sync(&mut self, direction: SortDirection) {
        let sort_col = self.view.cursor_col;
        let skip_header = self.header_mode;

        let permutation = match self.table.get_sort_permutation(sort_col, direction, skip_header) {
            Some(p) => p,
            None => {
                self.message = Some("Already sorted".to_string());
                return;
            }
        };

        self.table.apply_row_permutation(&permutation);
        let txn = Transaction::PermuteRows { permutation };
        self.history.record(txn);
        self.dirty = true;

        let sort_type = self.table.probe_column_type(sort_col, skip_header);
        let type_str = match sort_type {
            ColumnType::Numeric => "numeric",
            ColumnType::Text => "text",
        };
        let dir_str = match direction {
            SortDirection::Ascending => "ascending",
            SortDirection::Descending => "descending",
        };
        self.message = Some(format!("Sorted {} ({})", dir_str, type_str));
    }

    fn sort_by_row(&mut self, direction: SortDirection) {
        let sort_row = self.view.cursor_row;
        let skip_first = self.header_mode;

        // Get the sort permutation (returns None if already sorted)
        let permutation = match self.table.get_col_sort_permutation(sort_row, direction, false) {
            Some(p) => p,
            None => {
                self.message = Some("Already sorted".to_string());
                return;
            }
        };

        // Apply the permutation
        self.table.apply_col_permutation(&permutation);

        // Record the permutation transaction (memory-efficient: only stores indices)
        let txn = Transaction::PermuteCols { permutation };
        self.history.record(txn);
        self.dirty = true;

        let sort_type = self.table.probe_row_type(sort_row, skip_first);
        let type_str = match sort_type {
            ColumnType::Numeric => "numeric",
            ColumnType::Text => "text",
        };
        let dir_str = match direction {
            SortDirection::Ascending => "ascending",
            SortDirection::Descending => "descending",
        };
        self.message = Some(format!("Columns sorted {} ({})", dir_str, type_str));
    }
}

/// Sort key for background sorting
#[derive(Clone, PartialEq)]
enum SortKey {
    Numeric(f64),
    Text(String),
}

impl Eq for SortKey {}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (SortKey::Numeric(a), SortKey::Numeric(b)) => {
                match (a.is_nan(), b.is_nan()) {
                    (true, true) => std::cmp::Ordering::Equal,
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    (false, false) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
                }
            }
            (SortKey::Text(a), SortKey::Text(b)) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        }
    }
}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
