use std::io;
use std::time::Duration;
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::rc::Rc;
use std::cell::RefCell;

use crossterm::event::{self, poll, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::numeric::calculator::Calculator;
use crate::ui::canvas::Canvas;
use crate::clipboard::Clipboard;
use crate::mode::command::{Command, CommandHandler};
use crate::mode::insert::InsertHandler;
use crate::input::{
    is_escape, KeyBuffer, KeyBufferResult, KeyResult, NavigationHandler
};
use crate::mode::search::SearchHandler;
use crate::mode::Mode;
use crate::table::operations;
use crate::plugin::PluginManager;
use crate::table::table::Table;
use crate::table::SortDirection;
use crate::table::tableview::TableView;
use crate::transaction::{History, Transaction};
use crate::ui;
use crate::fileio::FileIO;
use crate::ui::style::Style;
use crate::ui::progress::Progress;
use crate::table::rowmanager::RowManager;
use crate::util::ColumnType;
use crate::mode::visual::{VisualType, VisualHandler};
use crate::config::AppConfig;
use crate::mode::normal::NormalHandler;

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
    pub config: Rc<RefCell<AppConfig>>,
    pub row_manager: Rc<RefCell<RowManager>>,
    pub dirty: bool,
    pub calling_mode: Option<Mode>,
    pub message: Option<String>,
    pub should_quit: bool,
    pub header_mode: bool,
    pub precision: Option<usize>,  // Display precision for numbers (None = auto)
    pub progress: Option<(String, Progress)>,  // Optional progress indicator (operation name, progress)
    pub canvas: Canvas,  // Canvas overlay for displaying text/images
    pub(crate) pending_op: Option<PendingOp>,  // Pending operation to execute after next render
    // Background task handling
    pub(crate) bg_receiver: Option<Receiver<BackgroundResult>>,
    #[allow(dead_code)]
    pub(crate) bg_handle: Option<JoinHandle<()>>,
    // Mode handlers
    key_buffer: KeyBuffer,
    pub(crate) nav_handler: NavigationHandler,
    pub search_handler: SearchHandler,
    pub insert_handler: InsertHandler,
    pub(crate) command_handler: CommandHandler,
    normal_handler: NormalHandler,
    // Plugin system
    pub(crate) plugin_manager: PluginManager,
}

impl App {
    pub fn new(table: Table, file_io: FileIO) -> Self {
        let row_manager = Rc::new(RefCell::new(RowManager::new()));
        let view = TableView::new(row_manager.clone());
        let clipboard = Clipboard::new();

        let mut plugin_manager = PluginManager::new();
        let _ = plugin_manager.load_plugins();

        let config = Rc::new(RefCell::new(AppConfig::new()));
        let key_buffer = KeyBuffer::new(config.clone());

        Self {
            table,
            view,
            clipboard,
            history: History::new(),
            style: Style::new(),
            mode: Mode::Normal,
            file_io,
            config,
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
            key_buffer,
            nav_handler: NavigationHandler::new(),
            search_handler: SearchHandler::new(),
            insert_handler: InsertHandler::new(),
            command_handler: CommandHandler::new(),
            normal_handler: NormalHandler::new(),
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
                let calc = Calculator::with_plugins(&self.table, self.header_mode, &self.plugin_manager);
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

            terminal.draw(|f| ui::ui::render(f, self, self.row_manager.clone()))?;

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
    pub(crate) fn execute(&mut self, txn: Transaction) {
        if matches!(txn, Transaction::Undo) {
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
            } else {
                self.message = Some(String::from("Cannot undo."));
            }
        } else if matches!(txn, Transaction::Redo) {
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
            } else {
                self.message = Some(String::from("Cannot redo."));
            }
        } else {
            txn.apply(&mut self.table);
            self.history.record(txn);
            self.dirty = true;
        }
    }

    /// Execute and return to normal mode
    pub(crate) fn execute_and_finish(&mut self, txn: Transaction) {
        self.execute(txn);
        self.finish_edit();
    }

    /// Return to normal mode and update column widths
    pub(crate) fn finish_edit(&mut self) {
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
                //self.handle_normal_key(key, count);
                let result = self.normal_handler.handle_key(key, 
                                                            &mut self.view,
                                                            &mut self.table,
                                                            count,
                                                            &self.nav_handler,
                                                            self.row_manager.borrow().is_filtered,
                                                            &mut self.clipboard,
                                                            &mut self.search_handler
                                                            );
                self.process_key_result(result);
            }
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
}
