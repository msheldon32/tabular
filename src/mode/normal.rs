use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

use crate::input::{KeyResult, NavigationHandler};
use crate::table::table::Table;
use crate::table::tableview::TableView;
use crate::mode::Mode;
use crate::transaction::Transaction;
use crate::clipboard::Clipboard;
use crate::mode::search::SearchHandler;

pub struct NormalHandler {
}

impl NormalHandler {
    pub fn new() -> Self {
        Self {
        }
    }

    pub fn handle_key(&mut self, 
                  key: KeyEvent,
                  view: &mut TableView,
                  table: &mut Table,
                  _count: usize, 
                  nav_handler: &NavigationHandler,
                  is_filtered: bool,
                  clipboard: &mut Clipboard,
                  search_handler: &mut SearchHandler
                  ) -> KeyResult {
        // Handle navigation (hjkl already handled by KeyBuffer with count)
        nav_handler.handle(key, view, table);

        match key.code {
            KeyCode::Char('i') => {
                return KeyResult::SwitchMode(Mode::Insert);
            }
            KeyCode::Char('V') => {
                view.set_support();
                return KeyResult::SwitchMode(Mode::VisualRow);
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                view.set_support();
                return KeyResult::SwitchMode(Mode::VisualCol);
            }
            KeyCode::Char('v') => {
                view.set_support();
                return KeyResult::SwitchMode(Mode::Visual);
            }
            KeyCode::Char(':') => {
                return KeyResult::SwitchMode(Mode::Command);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return KeyResult::Quit;
            }
            KeyCode::Char('o') => {
                if is_filtered {
                    return KeyResult::Message("Adding rows is forbidden in filtered views.".to_string());
                }
                let txn = Transaction::InsertRow { idx: view.cursor_row + 1 };
                view.cursor_row += 1;
                view.scroll_to_cursor();

                return KeyResult::Execute(txn);
            }
            KeyCode::Char('O') => {
                if is_filtered {
                    return KeyResult::Message("Adding rows is forbidden in filtered views.".to_string());
                }
                let txn = Transaction::InsertRow { idx: view.cursor_row };
                view.scroll_to_cursor();
                
                return KeyResult::Execute(txn);
            }
            KeyCode::Char('p') => {
                let (message, txn_opt) = clipboard.paste_as_transaction(
                    view.cursor_row,
                    view.cursor_col,
                    &table,
                );
                if let Some(txn) = txn_opt {
                    return KeyResult::Execute(txn);
                }
                return KeyResult::Message(message);
            }
            KeyCode::Char('a') => {
                let txn = Transaction::InsertCol { idx: view.cursor_col };
                return KeyResult::Execute(txn);
            }
            KeyCode::Char('A') => {
                let txn = Transaction::InsertCol { idx: view.cursor_col + 1 };
                return KeyResult::Execute(txn);
            }
            KeyCode::Char('X') => {
                if let Some(col_data) = table.get_col_cloned(view.cursor_col) {
                    let txn = Transaction::DeleteCol {
                        idx: view.cursor_col,
                        data: col_data,
                    };
                    view.clamp_cursor(table);
                    return KeyResult::Execute(txn);
                }
                return KeyResult::Message(String::from("Cannot remove column."));
            }
            KeyCode::Char('x') => {
                let old_value = crate::table::operations::current_cell(view, table).clone();
                clipboard.store_deleted(crate::clipboard::RegisterContent{
                    data: vec![vec![old_value.clone()]],
                    anchor: crate::clipboard::PasteAnchor::Cursor
                });

                let txn = Transaction::SetCell {
                    row: view.cursor_row,
                    col: view.cursor_col,
                    old_value,
                    new_value: String::new(),
                };
                return KeyResult::Execute(txn);
            }
            KeyCode::Char('u') => {
                return KeyResult::Execute(Transaction::Undo);
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return KeyResult::Execute(Transaction::Redo);
            }
            KeyCode::Char('/') => {
                return KeyResult::SwitchMode(Mode::Search);
            }
            KeyCode::Char('n') => {
                if let Some(msg) = search_handler.goto_next(view) {
                    return KeyResult::Message(msg);
                }
            }
            KeyCode::Char('N') => {
                if let Some(msg) = search_handler.goto_prev(view) {
                    return KeyResult::Message(msg);
                }
            }
            _ => {}
        }
        KeyResult::Continue
    }
}
