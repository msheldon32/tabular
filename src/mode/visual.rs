use crossterm::event::{KeyCode, KeyEvent};

use crate::mode::Mode;
use crate::transaction::Transaction;
use crate::input::{KeyResult, KeyBufferResult, SequenceAction, is_escape, NavigationHandler, KeyBuffer};
use crate::table::table::Table;
use crate::table::tableview::TableView;
use crate::clipboard::{Clipboard, RegisterContent, PasteAnchor};
use crate::numeric::format::{format_scientific, format_percentage, format_currency, format_commas, format_default, parse_numeric};

/// Selection information for visual mode
#[derive(Clone, Debug, Default)]
pub struct SelectionInfo {
    pub mode: Mode,
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

/// Visual selection mode types
#[derive(Clone, Copy, PartialEq)]
pub enum VisualType {
    Cell,
    Row,
    Col,
}

/// Format operation types for visual mode formatting
#[derive(Clone, Copy, PartialEq)]
pub enum FormatOp {
    Default,
    Commas,
    Currency,
    Scientific,
    Percentage,
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
                    SequenceAction::Delete => return self.handle_delete(view, table, clipboard),
                    // Format actions
                    SequenceAction::FormatDefault => {
                        return self.handle_format(view, table, FormatOp::Default);
                    }
                    SequenceAction::FormatCommas => {
                        return self.handle_format(view, table, FormatOp::Commas);
                    }
                    SequenceAction::FormatCurrency => {
                        return self.handle_format(view, table, FormatOp::Currency);
                    }
                    SequenceAction::FormatScientific => {
                        return self.handle_format(view, table, FormatOp::Scientific);
                    }
                    SequenceAction::FormatPercentage => {
                        return self.handle_format(view, table, FormatOp::Percentage);
                    }
                    SequenceAction::SelectRegister(reg) => {
                        if let Err(e) = clipboard.select_register(reg) {
                            return KeyResult::Message(e);
                        }
                        // Don't return - stay in visual mode, next action uses this register
                    }
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
                    KeyCode::Char('x') => self.handle_delete(view, table, clipboard),
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
                    if view.row_manager.borrow().is_filtered {
                        let good_rows = (start_row..=end_row).filter(|&i| view.row_manager.borrow().is_row_live(i))
                                            .map(|i| span[i.saturating_sub(start_row)].clone()).collect();

                        clipboard.yank_span(good_rows);
                    } else {
                        clipboard.yank_span(span);
                    }
                }
            }
            VisualType::Row => {
                // Yank all selected rows using bulk get
                let count = end_row - start_row + 1;
                let rows = if view.row_manager.borrow().is_filtered {
                    let mut vec = Vec::new();
                    for idx in (start_row..=end_row).filter(|&i| view.row_manager.borrow().is_row_live(i)) {
                        vec.push(table.get_row_cloned(idx).unwrap());
                    }
                    vec
                } else {
                    table.get_rows_cloned(start_row, count)
                };
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

    fn handle_delete(&self, view: &TableView, table: &Table, clipboard: &mut Clipboard) -> KeyResult {
        if view.row_manager.borrow().is_filtered {
            return KeyResult::Message("Deleting rows is forbidden in filtered views.".to_string());
        }
        let (start_row, end_row, start_col, end_col) = view.get_selection_bounds();

        match self.visual_type {
            VisualType::Cell => {
                // Clear cell contents
                let old_data = table.get_span(start_row, end_row, start_col, end_col)
                    .unwrap_or_default();

                clipboard.store_deleted(RegisterContent {
                    data: old_data.clone(),
                    anchor: PasteAnchor::Cursor
                });

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
                // Delete entire rows using bulk operation
                let count = end_row - start_row + 1;
                let rows = table.get_rows_cloned(start_row, count);

                clipboard.store_deleted(RegisterContent {
                    data: rows.clone(),
                    anchor: PasteAnchor::RowStart
                });
                if rows.is_empty() {
                    KeyResult::Finish
                } else {
                    KeyResult::ExecuteAndFinish(Transaction::DeleteRowsBulk {
                        idx: start_row,
                        data: rows,
                    })
                }
            }
            VisualType::Col => {
                // Delete entire columns
                let cols = table.get_cols_cloned(start_col, end_col);
                clipboard.store_deleted(RegisterContent {
                    data: cols,
                    anchor: PasteAnchor::RowStart
                });
                let txns: Vec<Transaction> = (start_col..=end_col)
                    .filter_map(|c| {
                        table.get_col_cloned(c).map(|data| Transaction::DeleteCol {
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
        if view.row_manager.borrow().is_filtered {
            return KeyResult::Message("Drag is forbidden in filtered views.".to_string());
        }
        match self.visual_type {
            VisualType::Cell | VisualType::Row => {
                let txn = create_drag_down_txn(view, table, self.visual_type == VisualType::Row);
                KeyResult::ExecuteAndFinish(txn)
            }
            VisualType::Col => KeyResult::Continue, // Not applicable
        }
    }

    fn handle_drag_right(&self, view: &TableView, table: &Table) -> KeyResult {
        if view.row_manager.borrow().is_filtered {
            return KeyResult::Message("Drag is forbidden in filtered views.".to_string());
        }
        match self.visual_type {
            VisualType::Cell | VisualType::Col => {
                let txn = create_drag_right_txn(view, table, self.visual_type == VisualType::Col);
                KeyResult::ExecuteAndFinish(txn)
            }
            VisualType::Row => KeyResult::Continue, // Not applicable
        }
    }

    fn handle_format(&self, view: &TableView, table: &Table, op: FormatOp) -> KeyResult {
        let (sel_start_row, sel_end_row, sel_start_col, sel_end_col) = view.get_selection_bounds();

        // Expand selection based on visual type
        let (start_row, end_row, start_col, end_col) = match self.visual_type {
            VisualType::Row => {
                // Full rows
                (sel_start_row, sel_end_row, 0, table.col_count().saturating_sub(1))
            }
            VisualType::Col => {
                // Full columns
                (0, table.row_count().saturating_sub(1), sel_start_col, sel_end_col)
            }
            VisualType::Cell => {
                // Just the selected cells
                (sel_start_row, sel_end_row, sel_start_col, sel_end_col)
            }
        };

        // Get the old data
        let old_data = table.get_span(start_row, end_row, start_col, end_col)
            .unwrap_or_default();

        // Apply format to each cell
        let new_data: Vec<Vec<String>> = old_data.iter()
            .map(|row| {
                row.iter()
                    .map(|cell| {
                        let formatted = match op {
                            FormatOp::Default => format_default(cell),
                            FormatOp::Commas => format_commas(cell),
                            FormatOp::Currency => format_currency(cell, '$'),
                            FormatOp::Scientific => format_scientific(cell, 2),
                            FormatOp::Percentage => format_percentage(cell, 0),
                        };
                        // If formatting failed (non-numeric), keep original value
                        formatted.unwrap_or_else(|| cell.clone())
                    })
                    .collect()
            })
            .collect();

        let txn = Transaction::SetSpan {
            row: start_row,
            col: start_col,
            old_data,
            new_data,
        };
        KeyResult::ExecuteAndFinish(txn)
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
