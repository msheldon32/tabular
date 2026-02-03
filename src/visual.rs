use crate::mode::Mode;

/// Selection information for visual mode
#[derive(Clone, Debug, Default)]
pub struct SelectionInfo {
    pub mode: Mode,
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}
