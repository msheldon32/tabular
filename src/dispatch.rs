//! Command dispatch and execution for App
//!
//! This module contains all command execution logic extracted from App,
//! including command dispatch, plugin execution, sorting, and replace operations.

use std::cmp;
use std::sync::mpsc;
use std::thread;

use crate::app::{App, BackgroundResult, PendingOp};
use crate::numeric::calculator::Calculator;
use crate::mode::command::{Command, ReplaceCommand, ReplaceScope};
use crate::mode::visual::SelectionInfo;
use crate::mode::Mode;
use crate::input::{KeyResult, SequenceAction};
use crate::plugin::{PluginAction, PluginContext};
use crate::table::SortDirection;
use crate::table::rowmanager::FilterType;
use crate::transaction::Transaction;
use crate::util::ColumnType;

impl App {
    pub fn execute_sequence_action(&mut self, action: SequenceAction, count: usize) {
        match action {
            SequenceAction::SelectRegister(reg) => {
                if let Err(e) = self.clipboard.select_register(reg) {
                    self.message = Some(e);
                }
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
                    use crate::clipboard::RegisterContent;
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

                let cols: Vec<Vec<String>> = (0..self.table.row_count())
                    .map(|r| {
                        (start_col..end_col)
                            .map(|c| self.table.get_cell(r, c).cloned().unwrap_or_default())
                            .collect()
                    })
                    .collect();
                if !cols.is_empty() {
                    use crate::clipboard::RegisterContent;
                    self.clipboard.store_deleted(RegisterContent::from_cols(cols));
                }

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
                    use crate::clipboard::RegisterContent;
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
            SequenceAction::FormatDefault
            | SequenceAction::FormatCommas
            | SequenceAction::FormatCurrency
            | SequenceAction::FormatScientific
            | SequenceAction::FormatPercentage => {}
        }
    }

    pub fn process_key_result(&mut self, result: KeyResult) {
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
                } else if mode == Mode::Insert {
                    let current = crate::table::operations::current_cell(&self.view, &self.table).clone();
                    self.insert_handler.start_edit(current);
                } else if mode == Mode::Search {
                    self.search_handler.start_search();
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

    pub fn execute_command(&mut self, cmd: Command) {
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
                let cell_count = self.table.row_count() * self.table.col_count();
                if cell_count >= 50_000 {
                    self.start_progress("Calculating", cell_count);
                    self.pending_op = Some(PendingOp::Calc { formula_count: cell_count });
                } else {
                    let calc = Calculator::with_plugins(&self.table, self.header_mode, &self.plugin_manager);
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
                use crate::ui::style::Theme;
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
                use crate::ui::style::Theme;
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
                let old_state = self.row_manager.borrow().snapshot();
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
                let new_state = self.row_manager.borrow().snapshot();
                let txn = Transaction::SetFilter { old_state, new_state };
                self.history.record(txn);
            }
            Command::Canvas => {
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

    pub fn get_selection_info(&self) -> SelectionInfo {
        let mode = if self.mode == Mode::Command {
            self.calling_mode.unwrap_or(self.mode)
        } else {
            self.mode
        };
        SelectionInfo {
            mode,
            start_row: cmp::min(self.view.support_row, self.view.cursor_row),
            end_row: cmp::max(self.view.support_row, self.view.cursor_row),
            start_col: cmp::min(self.view.support_col, self.view.cursor_col),
            end_col: cmp::max(self.view.support_col, self.view.cursor_col),
        }
    }

    pub fn execute_plugin(&mut self, name: &str, args: &[String]) {
        let ctx = PluginContext {
            cursor_row: self.view.cursor_row,
            cursor_col: self.view.cursor_col,
            row_count: self.table.row_count(),
            col_count: self.table.col_count(),
            selection: self.get_selection_info()
        };

        let table = &self.table;
        let get_cell = |row: usize, col: usize| -> Option<String> {
            table.get_cell(row, col).cloned()
        };

        match self.plugin_manager.execute(name, args, &ctx, get_cell) {
            Ok(result) => {
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
                        PluginAction::PromptRequest { question, default: _ } => {
                            // TODO: Implement full prompt UI with deferred execution
                            self.message = Some(format!("Prompt requested: {} (use :cmd args instead)", question));
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

    pub fn execute_replace(&mut self, cmd: ReplaceCommand) {
        let (row_range, col_range) = match cmd.scope {
            ReplaceScope::All => {
                (0..self.table.row_count(), 0..self.table.col_count())
            }
            ReplaceScope::Selection => {
                if self.calling_mode.map_or(false, |x| x.is_visual()) {
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

    pub fn sort_by_column(&mut self, direction: SortDirection) {
        if self.bg_receiver.is_some() {
            self.message = Some("Sort already in progress".to_string());
            return;
        }

        let sort_col = self.view.cursor_col;
        let skip_header = self.header_mode;
        let row_count = self.table.row_count();

        if row_count < 50_000 {
            self.sort_by_column_sync(direction);
            return;
        }

        let sort_type = self.table.probe_column_type(sort_col, skip_header);
        let col_data: Vec<String> = (0..row_count)
            .map(|row| {
                self.table.get_cell(row, sort_col)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();

        let progress = self.start_progress("Sorting", row_count);
        let (tx, rx) = mpsc::channel();
        self.bg_receiver = Some(rx);

        let handle = thread::spawn(move || {
            let start_row = if skip_header { 1 } else { 0 };
            let mut keyed: Vec<(usize, SortKey)> = Vec::with_capacity(row_count - start_row);

            for (i, row) in (start_row..row_count).enumerate() {
                let key = match sort_type {
                    ColumnType::Numeric => {
                        let val = crate::numeric::format::parse_numeric(col_data[row].trim())
                            .unwrap_or(f64::NAN);
                        SortKey::Numeric(val)
                    }
                    ColumnType::Text => {
                        SortKey::Text(col_data[row].to_lowercase())
                    }
                };
                keyed.push((row, key));

                if i % 10000 == 0 {
                    progress.set(i);
                }
            }

            progress.set(row_count / 2);

            keyed.sort_unstable_by(|(idx_a, key_a), (idx_b, key_b)| {
                let cmp = key_a.cmp(key_b);
                match direction {
                    SortDirection::Ascending => cmp.then(idx_a.cmp(idx_b)),
                    SortDirection::Descending => cmp.reverse().then(idx_a.cmp(idx_b)),
                }
            });

            progress.set(row_count);

            let mut permutation: Vec<usize> = if skip_header {
                vec![0]
            } else {
                Vec::new()
            };
            permutation.extend(keyed.into_iter().map(|(row, _)| row));

            let already_sorted = permutation.iter().enumerate().all(|(i, &idx)| i == idx);
            if already_sorted {
                let _ = tx.send(BackgroundResult::SortComplete {
                    permutation: Vec::new(),
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

    pub fn sort_by_row(&mut self, direction: SortDirection) {
        let sort_row = self.view.cursor_row;
        let skip_first = self.header_mode;

        let permutation = match self.table.get_col_sort_permutation(sort_row, direction, false) {
            Some(p) => p,
            None => {
                self.message = Some("Already sorted".to_string());
                return;
            }
        };

        self.table.apply_col_permutation(&permutation);
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
pub enum SortKey {
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
