use crate::table::Table;
use crate::transaction::Transaction;

/// Where yanked data should be anchored when pasting
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PasteAnchor {
    /// Paste at cursor position (for visual selections)
    Cursor,
    /// Paste starting at column 0 (for row yanks)
    RowStart,
    /// Paste starting at row 0 (for column yanks)
    ColStart,
}

pub struct Clipboard {
    pub yanked_row: Option<Vec<String>>,
    pub yanked_col: Option<Vec<String>>,
    pub yanked_span: Option<Vec<Vec<String>>>,
    pub paste_anchor: PasteAnchor,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            yanked_row: None,
            yanked_col: None,
            yanked_span: None,
            paste_anchor: PasteAnchor::Cursor,
        }
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

            // Determine paste position based on anchor
            let (paste_row, paste_col, msg) = match self.paste_anchor {
                PasteAnchor::RowStart => (cursor_row, 0, format!("{} row(s) pasted", rows)),
                PasteAnchor::ColStart => (0, cursor_col, format!("{} column(s) pasted", cols)),
                PasteAnchor::Cursor => (cursor_row, cursor_col, "Span pasted".to_string()),
            };

            let old_data = table.get_span(
                paste_row,
                paste_row + rows - 1,
                paste_col,
                paste_col + cols - 1,
            ).unwrap_or_default();
            let txn = Transaction::SetSpan {
                row: paste_row,
                col: paste_col,
                old_data,
                new_data: span_data.clone(),
            };
            return (msg, Some(txn));
        }

        ("Nothing to paste".to_string(), None)
    }

    pub fn yank_row(&mut self, row: Vec<String>) {
        self.yanked_row = Some(row);
        self.yanked_col = None;
        self.yanked_span = None;
        self.paste_anchor = PasteAnchor::RowStart;
    }

    pub fn yank_col(&mut self, col: Vec<String>) {
        self.yanked_col = Some(col);
        self.yanked_row = None;
        self.yanked_span = None;
        self.paste_anchor = PasteAnchor::ColStart;
    }

    pub fn yank_span(&mut self, span: Vec<Vec<String>>) {
        self.yanked_col = None;
        self.yanked_row = None;
        self.yanked_span = Some(span);
        // Default to cursor anchor for visual selections
        self.paste_anchor = PasteAnchor::Cursor;
    }

    /// Yank multiple rows (paste will start at column 0)
    pub fn yank_rows(&mut self, rows: Vec<Vec<String>>) {
        self.yanked_col = None;
        self.yanked_row = None;
        self.yanked_span = Some(rows);
        self.paste_anchor = PasteAnchor::RowStart;
    }

    /// Yank multiple columns (paste will start at row 0)
    pub fn yank_cols(&mut self, cols: Vec<Vec<String>>) {
        self.yanked_col = None;
        self.yanked_row = None;
        self.yanked_span = Some(cols);
        self.paste_anchor = PasteAnchor::ColStart;
    }

    /// Copy current yank to system clipboard as TSV
    pub fn to_system(&self) -> Result<String, String> {
        let data = if let Some(ref row) = self.yanked_row {
            vec![row.clone()]
        } else if let Some(ref col) = self.yanked_col {
            col.iter().map(|c| vec![c.clone()]).collect()
        } else if let Some(ref span) = self.yanked_span {
            span.clone()
        } else {
            return Err("Nothing to copy".to_string());
        };

        // Convert to TSV (tab-separated values)
        let tsv: String = data
            .iter()
            .map(|row| row.join("\t"))
            .collect::<Vec<_>>()
            .join("\n");

        copy_to_system_clipboard(&tsv)?;

        let rows = data.len();
        let cols = data.first().map(|r| r.len()).unwrap_or(0);
        Ok(format!("Copied {}x{} to system clipboard", rows, cols))
    }

    /// Paste from system clipboard (parses as TSV)
    pub fn from_system(&mut self) -> Result<String, String> {
        let text = paste_from_system_clipboard()?;

        if text.is_empty() {
            return Err("System clipboard is empty".to_string());
        }

        // Parse TSV (also handles single values)
        let span: Vec<Vec<String>> = text
            .lines()
            .map(|line| line.split('\t').map(|s| s.to_string()).collect())
            .collect();

        let rows = span.len();
        let cols = span.first().map(|r| r.len()).unwrap_or(0);

        self.yanked_row = None;
        self.yanked_col = None;
        self.yanked_span = Some(span);
        self.paste_anchor = PasteAnchor::Cursor;

        Ok(format!("Yanked {}x{} from system clipboard", rows, cols))
    }
}

/// Copy text to system clipboard using platform-appropriate method
fn copy_to_system_clipboard(text: &str) -> Result<(), String> {
    // Try command-line tools first on Linux (more reliable with terminal apps)
    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        use std::io::Write;

        // Try wl-copy (Wayland) first, then xclip (X11)
        let commands = [
            ("wl-copy", vec![]),
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
        ];

        for (cmd, args) in commands {
            if let Ok(mut child) = Command::new(cmd)
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin);
                        if child.wait().map(|s| s.success()).unwrap_or(false) {
                            return Ok(());
                        }
                    }
                }
            }
        }

        return Err("No clipboard tool found (install xclip or wl-copy)".to_string());
    }

    // Use arboard on other platforms (macOS, Windows)
    #[cfg(not(target_os = "linux"))]
    {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| format!("Clipboard error: {}", e))?;
        clipboard
            .set_text(text)
            .map_err(|e| format!("Clipboard error: {}", e))?;
        Ok(())
    }
}

/// Paste text from system clipboard using platform-appropriate method
fn paste_from_system_clipboard() -> Result<String, String> {
    // Try command-line tools first on Linux
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let commands = [
            ("wl-paste", vec!["--no-newline"]),
            ("xclip", vec!["-selection", "clipboard", "-o"]),
            ("xsel", vec!["--clipboard", "--output"]),
        ];

        for (cmd, args) in commands {
            if let Ok(output) = Command::new(cmd)
                .args(&args)
                .output()
            {
                if output.status.success() {
                    return String::from_utf8(output.stdout)
                        .map_err(|_| "Clipboard contains invalid UTF-8".to_string());
                }
            }
        }

        return Err("No clipboard tool found (install xclip or wl-copy)".to_string());
    }

    // Use arboard on other platforms
    #[cfg(not(target_os = "linux"))]
    {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| format!("Clipboard error: {}", e))?;
        clipboard
            .get_text()
            .map_err(|e| format!("Clipboard error: {}", e))
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

    #[test]
    fn test_paste_span_extends_table() {
        let mut clipboard = Clipboard::new();
        clipboard.yank_span(vec![
            vec!["1".to_string(), "2".to_string(), "3".to_string()],
            vec!["4".to_string(), "5".to_string(), "6".to_string()],
            vec!["7".to_string(), "8".to_string(), "9".to_string()],
        ]);

        // Small 2x2 table
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        // Paste at position that will extend beyond table
        let (msg, txn) = clipboard.paste_as_transaction(1, 1, &table);

        assert_eq!(msg, "Span pasted");
        assert!(txn.is_some());

        let txn = txn.unwrap();
        let mut table = table;
        txn.apply(&mut table);

        // Table should now be expanded
        assert!(table.row_count() >= 4); // rows 1,2,3 + original
        assert!(table.col_count() >= 4); // cols 1,2,3 + original

        // Check pasted values
        assert_eq!(table.cells[1][1], "1");
        assert_eq!(table.cells[1][2], "2");
        assert_eq!(table.cells[1][3], "3");
        assert_eq!(table.cells[2][1], "4");
        assert_eq!(table.cells[3][3], "9");

        // Original values preserved where not overwritten
        assert_eq!(table.cells[0][0], "a");
        assert_eq!(table.cells[0][1], "b");
        assert_eq!(table.cells[1][0], "c");
    }
}
