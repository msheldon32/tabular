use std::sync::mpsc;
use std::thread;

use crate::table::table::Table;
use crate::viewstate::{BackgroundResult, PendingOp};
use crate::table::tableview::TableView;
use crate::table::SortDirection;
use crate::transaction::transaction::Transaction;
use crate::util::ColumnType;
use crate::viewstate::ViewState;

// === Cell Access ===
/// Get current cell content
pub fn current_cell<'a>(view: &TableView, table: &'a Table) -> &'a String {
    table.get_cell(view.cursor_row, view.cursor_col)
        .expect("Cursor should be within bounds")
}


// === Sorting ===
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

    let sort_type = table.probe_column_type(sort_col, skip_header);
    let type_str = match sort_type {
        ColumnType::Numeric => "numeric",
        ColumnType::Text => "text",
    };
    let dir_str = match direction {
        SortDirection::Ascending => "ascending",
        SortDirection::Descending => "descending",
    };

    Some(txn)
}

pub fn sort_by_row(sort_row: usize, skip_first: bool, table: &mut Table, direction: SortDirection) -> Option<Transaction> {
    let permutation = match table.get_col_sort_permutation(sort_row, direction, false) {
        Some(p) => p,
        None => {
            return None;
        }
    };

    table.apply_col_permutation(&permutation);
    let txn = Transaction::PermuteCols { permutation };

    let sort_type = table.probe_row_type(sort_row, skip_first);
    let type_str = match sort_type {
        ColumnType::Numeric => "numeric",
        ColumnType::Text => "text",
    };
    let dir_str = match direction {
        SortDirection::Ascending => "ascending",
        SortDirection::Descending => "descending",
    };
    
    Some(txn)
}
