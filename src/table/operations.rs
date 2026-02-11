use std::sync::mpsc;
use std::thread;

use crate::table::table::Table;
use crate::viewstate::BackgroundResult;
use crate::table::tableview::TableView;
use crate::table::SortDirection;
use crate::transaction::transaction::Transaction;
use crate::util::ColumnType;
use crate::viewstate::ViewState;
use crate::mode::Mode;
use crate::mode::command::{ReplaceCommand, ReplaceScope};

// === Cell Access ===
/// Get current cell content
pub fn current_cell<'a>(view: &TableView, table: &'a Table) -> &'a String {
    table.get_cell(view.cursor_row, view.cursor_col)
        .expect("Cursor should be within bounds")
}


// === Sorting ===
/// Sort key for background sorting
#[derive(Clone, PartialEq)]
#[allow(dead_code)]
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

pub fn sort_by_column(sort_col: usize, skip_header: bool, table: &mut Table, view_state: &mut ViewState, direction: SortDirection) -> Option<Transaction> {
    let row_count = table.row_count();

    if row_count < 50_000 {
        return sort_by_column_sync(sort_col, skip_header, table, direction);
    }

    let sort_type = table.probe_column_type(sort_col, skip_header);
    let col_data: Vec<String> = (0..row_count)
        .map(|row| {
            table.get_cell(row, sort_col)
                .cloned()
                .unwrap_or_default()
        })
        .collect();

    let progress = view_state.start_progress("Sorting", row_count);
    let (tx, rx) = mpsc::channel();
    view_state.bg_receiver = Some(rx);

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

    view_state.bg_handle = Some(handle);
    None
}

fn sort_by_column_sync(sort_col: usize, skip_header: bool, table: &mut Table, direction: SortDirection) -> Option<Transaction> {
    let permutation = match table.get_sort_permutation(sort_col, direction, skip_header) {
        Some(p) => p,
        None => {
            return None;
        }
    };

    table.apply_row_permutation(&permutation);
    let txn = Transaction::PermuteRows { permutation };

    Some(txn)
}

pub fn sort_by_row(sort_row: usize, table: &mut Table, direction: SortDirection) -> Option<Transaction> {
    let permutation = match table.get_col_sort_permutation(sort_row, direction, false) {
        Some(p) => p,
        None => {
            return None;
        }
    };

    table.apply_col_permutation(&permutation);
    let txn = Transaction::PermuteCols { permutation };
    
    Some(txn)
}


pub fn replace(cmd: ReplaceCommand, table: &mut Table, view: &mut TableView, calling_mode: Option<Mode>) -> (Option<Transaction>, Option<String>) {
    let (row_range, col_range) = match cmd.scope {
        ReplaceScope::All => {
            (0..table.row_count(), 0..table.col_count())
        }
        ReplaceScope::Selection => {
            if calling_mode.map_or(false, |x| x.is_visual()) {
                let (start_row, end_row) = if calling_mode != Some(Mode::VisualCol) {
                    (std::cmp::min(view.cursor_row, view.support_row),
                        std::cmp::max(view.cursor_row, view.support_row))
                } else {
                    (0, table.row_count()-1)
                };
                let (start_col, end_col) = if calling_mode != Some(Mode::VisualRow) {
                    (std::cmp::min(view.cursor_col, view.support_col),
                        std::cmp::max(view.cursor_col, view.support_col))
                } else {
                    (0, table.col_count()-1)
                };
                (start_row..end_row + 1, start_col..end_col + 1)
            } else {
                (view.cursor_row..view.cursor_row+1, view.cursor_col..view.cursor_col+1)
            }
        }
    };

    let mut replacements = 0;
    let mut txns: Vec<Transaction> = Vec::new();
    let mut found = false;

    for row in row_range.clone() {
        for col in col_range.clone() {
            if let Some(cell) = table.get_cell(row, col) {
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
        (None, Some(format!("Pattern not found: {}", cmd.pattern)))

    } else {
        (Some(Transaction::Batch(txns)), Some(format!("{} replacement(s) made", replacements)))
    }
}
