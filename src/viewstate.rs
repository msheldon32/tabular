use std::rc::Rc;
use std::cell::RefCell;
use std::thread::JoinHandle;
use std::sync::mpsc::{self, Receiver};

use crate::table::tableview::TableView;
use crate::table::rowmanager::RowManager;
use crate::ui::progress::Progress;
use crate::table::table::Table;
use crate::table::SortDirection;
use crate::transaction::history::History;
use crate::util::ColumnType;
use crate::transaction::transaction::Transaction;
use crate::ui::canvas::Canvas;
use crate::ui::style::Style;


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

pub struct ViewState {
    pub view: TableView,
    pub style: Style,
    pub row_manager: Rc<RefCell<RowManager>>,
    pub precision: Option<usize>,  // Display precision for numbers (None = auto)
    pub canvas: Canvas,  // Canvas overlay for displaying text/images
    pub progress: Option<(String, Progress)>,  // Optional progress indicator (operation name, progress)
    pub(crate) pending_op: Option<PendingOp>,  // Pending operation to execute after next render
    // Background task handling
    pub(crate) bg_receiver: Option<Receiver<BackgroundResult>>,
    #[allow(dead_code)]
    pub(crate) bg_handle: Option<JoinHandle<()>>,

    pub message: Option<String>
}

impl ViewState {
    pub fn new() -> Self {
        let row_manager = Rc::new(RefCell::new(RowManager::new()));
        let view = TableView::new(row_manager.clone());

        Self {
            style: Style::new(),
            view,
            row_manager,
            canvas: Canvas::new(),
            precision: None,
            progress: None,
            pending_op: None,
            bg_receiver: None,
            bg_handle: None,
            message: None
        }
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

    /// Check for and handle completed background operations
    pub fn poll_background_result(&mut self, table: &mut Table, history: &mut History) -> (Option<String>, bool) {
        if let Some(ref receiver) = self.bg_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    let output = self.handle_background_result(result, table, history);
                    self.bg_receiver = None;
                    self.bg_handle = None;
                    self.clear_progress();

                    return output;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still working, progress is updated by the background thread
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Thread died unexpectedly
                    self.bg_receiver = None;
                    self.bg_handle = None;
                    self.clear_progress();
                    return (Some("Operation failed".to_string()), false);
                }
            }
        }
        (None, false)
    }

    /// Handle a completed background operation
    pub fn handle_background_result(&mut self, result: BackgroundResult, table: &mut Table, history: &mut History) -> (Option<String>, bool) {
        match result {
            BackgroundResult::SortComplete { permutation, direction, sort_type, is_column_sort } => {
                // Empty permutation means already sorted
                if permutation.is_empty() {
                    return (Some("Already sorted".to_string()), false);
                }

                if is_column_sort {
                    table.apply_col_permutation(&permutation);
                    let txn = Transaction::PermuteCols { permutation };
                    history.record(txn);
                } else {
                    table.apply_row_permutation(&permutation);
                    let txn = Transaction::PermuteRows { permutation };
                    history.record(txn);
                }

                let type_str = match sort_type {
                    ColumnType::Numeric => "numeric",
                    ColumnType::Text => "text",
                };
                let dir_str = match direction {
                    SortDirection::Ascending => "ascending",
                    SortDirection::Descending => "descending",
                };
                if is_column_sort {
                    return (Some(format!("Columns sorted {} ({})", dir_str, type_str)), true);
                } else {
                    return (Some(format!("Sorted {} ({})", dir_str, type_str)), true);
                }
            }
        }
    }
}
