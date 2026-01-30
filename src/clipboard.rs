use crate::table::Table;
use crate::transaction::Transaction;

pub struct Clipboard {
    pub yanked_row: Option<Vec<String>>,
    pub yanked_col: Option<Vec<String>>,
    pub yanked_span: Option<Vec<Vec<String>>>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self { yanked_row: None, yanked_col: None, yanked_span: None }
    }

    pub fn paste_as_transaction(
        &self,
        cursor_row: usize,
        cursor_col: usize,
        table: &Table,
    ) -> (String, Option<Transaction>) {
        if let Some(ref row_data) = self.yanked_row {
            let old_data = table.get_row(cursor_row).unwrap_or_default();
            let txn = Transaction::SetSpan {
                row: cursor_row,
                col: 0,
                old_data: vec![old_data],
                new_data: vec![row_data.clone()],
            };
            return ("Row pasted".to_string(), Some(txn));
        }

        if let Some(ref col_data) = self.yanked_col {
            let old_data: Vec<Vec<String>> = (0..table.row_count())
                .map(|r| vec![table.get_cell(r, cursor_col).cloned().unwrap_or_default()])
                .collect();
            let new_data: Vec<Vec<String>> = col_data.iter()
                .map(|v| vec![v.clone()])
                .collect();
            let txn = Transaction::SetSpan {
                row: 0,
                col: cursor_col,
                old_data,
                new_data,
            };
            return ("Column pasted".to_string(), Some(txn));
        }

        if let Some(ref span_data) = self.yanked_span {
            let rows = span_data.len();
            let cols = span_data.first().map(|r| r.len()).unwrap_or(0);
            let old_data = table.get_span(
                cursor_row,
                cursor_row + rows - 1,
                cursor_col,
                cursor_col + cols - 1,
            ).unwrap_or_default();
            let txn = Transaction::SetSpan {
                row: cursor_row,
                col: cursor_col,
                old_data,
                new_data: span_data.clone(),
            };
            return ("Span pasted".to_string(), Some(txn));
        }

        ("Nothing to paste".to_string(), None)
    }

    pub fn yank_row(&mut self, row: Vec<String>) {
        self.yanked_row = Some(row);
        self.yanked_col = None;
        self.yanked_span = None;
    }

    pub fn yank_col(&mut self, col: Vec<String>) {
        self.yanked_col = Some(col);
        self.yanked_row = None;
        self.yanked_span = None;
    }

    pub fn yank_span(&mut self, span: Vec<Vec<String>>) {
        self.yanked_col = None;
        self.yanked_row = None;
        self.yanked_span = Some(span);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(data: Vec<Vec<&str>>) -> Table {
        Table {
            cells: data.into_iter()
                .map(|row| row.into_iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn test_clipboard_new() {
        let clipboard = Clipboard::new();
        assert!(clipboard.yanked_row.is_none());
        assert!(clipboard.yanked_col.is_none());
        assert!(clipboard.yanked_span.is_none());
    }

    #[test]
    fn test_yank_row_clears_others() {
        let mut clipboard = Clipboard::new();
        clipboard.yanked_col = Some(vec!["a".to_string()]);
        clipboard.yanked_span = Some(vec![vec!["b".to_string()]]);

        clipboard.yank_row(vec!["x".to_string(), "y".to_string()]);

        assert_eq!(clipboard.yanked_row, Some(vec!["x".to_string(), "y".to_string()]));
        assert!(clipboard.yanked_col.is_none());
        assert!(clipboard.yanked_span.is_none());
    }

    #[test]
    fn test_yank_col_clears_others() {
        let mut clipboard = Clipboard::new();
        clipboard.yanked_row = Some(vec!["a".to_string()]);
        clipboard.yanked_span = Some(vec![vec!["b".to_string()]]);

        clipboard.yank_col(vec!["x".to_string(), "y".to_string()]);

        assert!(clipboard.yanked_row.is_none());
        assert_eq!(clipboard.yanked_col, Some(vec!["x".to_string(), "y".to_string()]));
        assert!(clipboard.yanked_span.is_none());
    }

    #[test]
    fn test_yank_span_clears_others() {
        let mut clipboard = Clipboard::new();
        clipboard.yanked_row = Some(vec!["a".to_string()]);
        clipboard.yanked_col = Some(vec!["b".to_string()]);

        clipboard.yank_span(vec![vec!["x".to_string()]]);

        assert!(clipboard.yanked_row.is_none());
        assert!(clipboard.yanked_col.is_none());
        assert_eq!(clipboard.yanked_span, Some(vec![vec!["x".to_string()]]));
    }

    #[test]
    fn test_paste_as_transaction_nothing() {
        let clipboard = Clipboard::new();
        let table = make_table(vec![vec!["a"]]);

        let (msg, txn) = clipboard.paste_as_transaction(0, 0, &table);

        assert_eq!(msg, "Nothing to paste");
        assert!(txn.is_none());
    }

    #[test]
    fn test_paste_as_transaction_row() {
        let mut clipboard = Clipboard::new();
        clipboard.yank_row(vec!["x".to_string(), "y".to_string()]);

        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let (msg, txn) = clipboard.paste_as_transaction(0, 0, &table);

        assert_eq!(msg, "Row pasted");
        assert!(txn.is_some());

        let txn = txn.unwrap();
        let mut table = table;
        txn.apply(&mut table);

        assert_eq!(table.cells[0], vec!["x", "y"]);
        assert_eq!(table.cells[1], vec!["c", "d"]);
    }

    #[test]
    fn test_paste_as_transaction_col() {
        let mut clipboard = Clipboard::new();
        clipboard.yank_col(vec!["x".to_string(), "y".to_string()]);

        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let (msg, txn) = clipboard.paste_as_transaction(0, 1, &table);

        assert_eq!(msg, "Column pasted");
        assert!(txn.is_some());

        let txn = txn.unwrap();
        let mut table = table;
        txn.apply(&mut table);

        assert_eq!(table.cells[0], vec!["a", "x"]);
        assert_eq!(table.cells[1], vec!["c", "y"]);
    }

    #[test]
    fn test_paste_as_transaction_span() {
        let mut clipboard = Clipboard::new();
        clipboard.yank_span(vec![
            vec!["1".to_string(), "2".to_string()],
            vec!["3".to_string(), "4".to_string()],
        ]);

        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        let (msg, txn) = clipboard.paste_as_transaction(0, 0, &table);

        assert_eq!(msg, "Span pasted");
        assert!(txn.is_some());

        let txn = txn.unwrap();
        let mut table = table;
        txn.apply(&mut table);

        assert_eq!(table.cells[0], vec!["1", "2", "c"]);
        assert_eq!(table.cells[1], vec!["3", "4", "f"]);
        assert_eq!(table.cells[2], vec!["g", "h", "i"]);
    }

    #[test]
    fn test_paste_as_transaction_span_offset() {
        let mut clipboard = Clipboard::new();
        clipboard.yank_span(vec![
            vec!["x".to_string()],
        ]);

        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);

        let (_, txn) = clipboard.paste_as_transaction(1, 2, &table);

        let txn = txn.unwrap();
        let mut table = table;
        txn.apply(&mut table);

        assert_eq!(table.cells[0], vec!["a", "b", "c"]);
        assert_eq!(table.cells[1], vec!["d", "e", "x"]);
    }

    #[test]
    fn test_paste_row_creates_correct_transaction() {
        let mut clipboard = Clipboard::new();
        clipboard.yank_row(vec!["new".to_string(), "row".to_string()]);

        let table = make_table(vec![
            vec!["old", "data"],
        ]);

        let (_, txn) = clipboard.paste_as_transaction(0, 0, &table);
        let txn = txn.unwrap();

        // Check that the transaction has the correct old_data for undo
        if let Transaction::SetSpan { old_data, new_data, .. } = txn {
            assert_eq!(old_data, vec![vec!["old".to_string(), "data".to_string()]]);
            assert_eq!(new_data, vec![vec!["new".to_string(), "row".to_string()]]);
        } else {
            panic!("Expected SetSpan transaction");
        }
    }

    #[test]
    fn test_paste_col_creates_correct_transaction() {
        let mut clipboard = Clipboard::new();
        clipboard.yank_col(vec!["x".to_string(), "y".to_string()]);

        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let (_, txn) = clipboard.paste_as_transaction(0, 0, &table);
        let txn = txn.unwrap();

        // Check that the transaction targets column 0
        if let Transaction::SetSpan { row, col, old_data, new_data } = txn {
            assert_eq!(row, 0);
            assert_eq!(col, 0);
            assert_eq!(old_data, vec![vec!["a".to_string()], vec!["c".to_string()]]);
            assert_eq!(new_data, vec![vec!["x".to_string()], vec!["y".to_string()]]);
        } else {
            panic!("Expected SetSpan transaction");
        }
    }
}
