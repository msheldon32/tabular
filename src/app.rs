use std::io;
use std::time::Duration;

use crossterm::event::{self, poll, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::calculator::Calculator;
use crate::clipboard::Clipboard;
use crate::command::{Command, ReplaceCommand};
use crate::input::{
    is_escape, CommandHandler, InsertHandler, KeyBuffer, KeyBufferResult, KeyResult,
    NavigationHandler, SearchHandler, SequenceAction, VisualHandler, VisualType,
};
use crate::mode::Mode;
use crate::table::{SortDirection, Table, TableView, SortType};
use crate::transaction::{History, Transaction};
use crate::ui;
use crate::fileio::FileIO;
use crate::style::Style;

pub struct App {
    pub table: Table,
    pub view: TableView,
    pub clipboard: Clipboard,
    pub history: History,
    pub style: Style,
    pub mode: Mode,
    pub file_io: FileIO,
    pub dirty: bool,
    pub has_selection: bool,
    pub message: Option<String>,
    pub should_quit: bool,
    pub header_mode: bool,
    // Mode handlers
    key_buffer: KeyBuffer,
    nav_handler: NavigationHandler,
    search_handler: SearchHandler,
    insert_handler: InsertHandler,
    command_handler: CommandHandler,
}

impl App {
    pub fn new(table: Table, file_io: FileIO) -> Self {
        let mut view = TableView::new();
        view.update_col_widths(&table);

        Self {
            table,
            view,
            clipboard: Clipboard::new(),
            history: History::new(),
            style: Style::new(),
            mode: Mode::Normal,
            file_io,
            dirty: false,
            has_selection: false,
            message: None,
            should_quit: false,
            header_mode: true,
            key_buffer: KeyBuffer::new(),
            nav_handler: NavigationHandler::new(),
            search_handler: SearchHandler::new(),
            insert_handler: InsertHandler::new(),
            command_handler: CommandHandler::new(),
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

    // === Key handling ===

    fn handle_key(&mut self, key: KeyEvent) {
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
            SequenceAction::MoveToTop => {
                self.view.move_to_top();
            }
            SequenceAction::MoveDown => {
                self.view.move_down_n(count, &self.table);
            }
            SequenceAction::MoveUp => {
                self.view.move_up_n(count);
            }
            SequenceAction::MoveLeft => {
                self.view.move_left_n(count);
            }
            SequenceAction::MoveRight => {
                self.view.move_right_n(count, &self.table);
            }
            SequenceAction::DeleteRow => {
                let start_row = self.view.cursor_row;
                let end_row = (start_row + count).min(self.table.row_count());
                let actual_count = end_row - start_row;

                // Yank the rows first
                let rows: Vec<Vec<String>> = (start_row..end_row)
                    .filter_map(|r| self.table.get_row(r))
                    .collect();
                if !rows.is_empty() {
                    self.clipboard.yank_rows(rows);
                }

                // Delete rows (always delete at start_row since indices shift)
                for _ in 0..actual_count {
                    if let Some(row_data) = self.table.get_row(start_row) {
                        let txn = Transaction::DeleteRow {
                            idx: start_row,
                            data: row_data,
                        };
                        self.execute(txn);
                    }
                }
                self.view.clamp_cursor(&self.table);
                self.view.update_col_widths(&self.table);
                let msg = if actual_count == 1 { "Row deleted".to_string() } else { format!("{} rows deleted", actual_count) };
                self.message = Some(msg);
            }
            SequenceAction::DeleteCol => {
                let start_col = self.view.cursor_col;
                let end_col = (start_col + count).min(self.table.col_count());
                let actual_count = end_col - start_col;

                // Yank the columns first
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

                // Delete columns (always delete at start_col since indices shift)
                for _ in 0..actual_count {
                    if let Some(col_data) = self.table.get_col(start_col) {
                        let txn = Transaction::DeleteCol {
                            idx: start_col,
                            data: col_data,
                        };
                        self.execute(txn);
                    }
                }
                self.view.clamp_cursor(&self.table);
                self.view.update_col_widths(&self.table);
                let msg = if actual_count == 1 { "Column deleted".to_string() } else { format!("{} columns deleted", actual_count) };
                self.message = Some(msg);
            }
            SequenceAction::YankRow => {
                let start_row = self.view.cursor_row;
                let end_row = (start_row + count).min(self.table.row_count());
                let actual_count = end_row - start_row;

                let rows: Vec<Vec<String>> = (start_row..end_row)
                    .filter_map(|r| self.table.get_row(r))
                    .collect();
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
                // In normal mode, yy yanks current row (like yr)
                if let Some(row) = self.table.get_row(self.view.cursor_row) {
                    self.clipboard.yank_rows(vec![row]);
                    self.message = Some("Row yanked".to_string());
                }
            }
            SequenceAction::Delete => {
                // In normal mode, dd deletes current row (like dr)
                if let Some(row_data) = self.table.get_row(self.view.cursor_row) {
                    self.clipboard.yank_rows(vec![row_data.clone()]);
                    let txn = Transaction::DeleteRow {
                        idx: self.view.cursor_row,
                        data: row_data,
                    };
                    self.execute(txn);
                    self.view.clamp_cursor(&self.table);
                    self.view.update_col_widths(&self.table);
                    self.message = Some("Row deleted".to_string());
                }
            }
            // Format actions are only meaningful in visual mode
            SequenceAction::FormatReduceDecimal
            | SequenceAction::FormatIncreaseDecimal
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
                let current = self.view.current_cell(&self.table).clone();
                self.insert_handler.start_edit(current);
            }
            KeyCode::Char('V') => {
                self.mode = Mode::VisualRow;
                self.has_selection = true;
                self.view.set_support();
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::VisualCol;
                self.has_selection = true;
                self.view.set_support();
            }
            KeyCode::Char('v') => {
                self.mode = Mode::Visual;
                self.has_selection = true;
                self.view.set_support();
            }
            KeyCode::Char(':') => {
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
            let old_value = self.view.current_cell(&self.table).clone();
            let txn = Transaction::SetCell {
                row: self.view.cursor_row,
                col: self.view.cursor_col,
                old_value,
                new_value: self.insert_handler.buffer.clone(),
            };
            self.execute_and_finish(txn);
            return;
        }

        self.insert_handler.handle_key(key, &self.view);
        self.view.expand_column(self.insert_handler.buffer.len());
    }

    fn handle_command_mode(&mut self, key: KeyEvent) {
        if is_escape(key) {
            self.mode = Mode::Normal;
            self.has_selection = false;
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
                self.has_selection = false;
            }
            KeyResult::SwitchMode(mode) => {
                self.mode = mode;
                if mode == Mode::Command {
                    self.command_handler.start();
                }
            }
            KeyResult::Execute(txn) => {
                self.execute(txn);
            }
            KeyResult::ExecuteAndFinish(txn) => {
                self.execute_and_finish(txn);
                self.has_selection = false;
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
                        self.view.update_col_widths(&self.table);
                        self.message = Some(format!("Evaluated {} formula(s)", count));
                    }
                    Err(e) => self.message = Some(format!("{}", e)),
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
            Command::SysPaste => {
                match self.clipboard.from_system() {
                    Ok(msg) => self.message = Some(msg),
                    Err(e) => self.message = Some(e),
                }
            }
            Command::Unknown(s) => self.message = Some(format!("Unknown command: {}", s)),
        }
        
        self.has_selection = false;
    }

    fn execute_replace(&mut self, cmd: ReplaceCommand) {
        use crate::command::ReplaceScope;

        // Determine which cells to search
        let (row_range, col_range) = match cmd.scope {
            ReplaceScope::All => {
                (0..self.table.row_count(), 0..self.table.col_count())
            }
            ReplaceScope::Selection => {
                if self.has_selection {
                    // Use the visual selection bounds (stored in view)
                    let start_row = std::cmp::min(self.view.cursor_row, self.view.support_row);
                    let end_row = std::cmp::max(self.view.cursor_row, self.view.support_row);
                    let start_col = std::cmp::min(self.view.cursor_col, self.view.support_col);
                    let end_col = std::cmp::max(self.view.cursor_col, self.view.support_col);
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
            self.view.update_col_widths(&self.table);
            self.message = Some(format!("{} replacement(s) made", replacements));
        }
    }

    fn sort_by_column(&mut self, direction: SortDirection) {
        let sort_col = self.view.cursor_col;
        let skip_header = self.header_mode;

        // Get the sort order
        let new_order = self.table.get_sorted_row_indices(sort_col, direction, skip_header);

        // Check if already sorted
        let already_sorted: bool = new_order.iter().enumerate().all(|(i, &idx)| i == idx);
        if already_sorted {
            self.message = Some("Already sorted".to_string());
            return;
        }

        // Capture old state for undo
        let old_data = self.table.cells.clone();

        // Perform the reorder
        self.table.reorder_rows(&new_order);

        // Create transaction for undo
        let new_data = self.table.cells.clone();
        let txn = Transaction::SetSpan {
            row: 0,
            col: 0,
            old_data,
            new_data,
        };
        self.history.record(txn);
        self.dirty = true;
        self.view.update_col_widths(&self.table);

        let sort_type = self.table.probe_column_type(sort_col, skip_header);
        let type_str = match sort_type {
            SortType::Numeric => "numeric",
            SortType::Text => "text",
        };
        let dir_str = match direction {
            SortDirection::Ascending => "ascending",
            SortDirection::Descending => "descending",
        };
        self.message = Some(format!("Sorted {} ({})", dir_str, type_str));
    }

    fn sort_by_row(&mut self, direction: SortDirection) {
        let sort_row = self.view.cursor_row;
        let skip_first = self.header_mode; // Optionally skip first column like row labels

        // Get the sort order
        let new_order = self.table.get_sorted_col_indices(sort_row, direction, false);

        // Check if already sorted
        let already_sorted: bool = new_order.iter().enumerate().all(|(i, &idx)| i == idx);
        if already_sorted {
            self.message = Some("Already sorted".to_string());
            return;
        }

        // Capture old state for undo
        let old_data = self.table.cells.clone();

        // Perform the reorder
        self.table.reorder_cols(&new_order);

        // Create transaction for undo
        let new_data = self.table.cells.clone();
        let txn = Transaction::SetSpan {
            row: 0,
            col: 0,
            old_data,
            new_data,
        };
        self.history.record(txn);
        self.dirty = true;
        self.view.update_col_widths(&self.table);

        let sort_type = self.table.probe_row_type(sort_row, skip_first);
        let type_str = match sort_type {
            SortType::Numeric => "numeric",
            SortType::Text => "text",
        };
        let dir_str = match direction {
            SortDirection::Ascending => "ascending",
            SortDirection::Descending => "descending",
        };
        self.message = Some(format!("Columns sorted {} ({})", dir_str, type_str));
    }
}
