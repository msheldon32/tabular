use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use crate::table::Table;
use crate::transaction::Transaction;
use crate::rowmanager::RowManager;

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

/// Content stored in a register
#[derive(Clone, Debug)]
pub struct RegisterContent {
    /// The actual data (2D for spans, single row/col, etc.)
    pub data: Vec<Vec<String>>,
    /// How to anchor when pasting
    pub anchor: PasteAnchor,
}

impl RegisterContent {
    pub fn new(data: Vec<Vec<String>>, anchor: PasteAnchor) -> Self {
        Self { data, anchor }
    }

    pub fn from_row(row: Vec<String>) -> Self {
        Self {
            data: vec![row],
            anchor: PasteAnchor::RowStart,
        }
    }

    pub fn from_rows(rows: Vec<Vec<String>>) -> Self {
        Self {
            data: rows,
            anchor: PasteAnchor::RowStart,
        }
    }

    pub fn from_col(col: Vec<String>) -> Self {
        Self {
            data: col.into_iter().map(|c| vec![c]).collect(),
            anchor: PasteAnchor::ColStart,
        }
    }

    pub fn from_cols(cols: Vec<Vec<String>>) -> Self {
        Self {
            data: cols,
            anchor: PasteAnchor::ColStart,
        }
    }

    pub fn from_span(span: Vec<Vec<String>>) -> Self {
        Self {
            data: span,
            anchor: PasteAnchor::Cursor,
        }
    }
}

/// Vim-style register system
///
/// Supported registers:
/// - `"` (unnamed) - default register, used when no register specified
/// - `a`-`z` - named registers for user storage
/// - `0` - yank register, stores last yank (not affected by delete)
/// - `_` - black hole register, discards everything written to it
/// - `+` - system clipboard register
pub struct Clipboard {
    /// Named registers (a-z) and special registers
    registers: HashMap<char, RegisterContent>,
    /// The unnamed register (default)
    unnamed: Option<RegisterContent>,
    /// Yank register (register 0) - last yank, unaffected by deletes
    yank_register: Option<RegisterContent>,
    /// Currently selected register for next operation (None = unnamed)
    selected: Option<char>,
    row_manager: Rc<RefCell<RowManager>>
}

impl Clipboard {
    pub fn new(row_manager: Rc<RefCell<RowManager>>) -> Self {
        Self {
            registers: HashMap::new(),
            unnamed: None,
            yank_register: None,
            selected: None,
            row_manager
        }
    }

    /// Select a register for the next yank/paste operation
    /// Returns error message if register is invalid
    pub fn select_register(&mut self, reg: char) -> Result<(), String> {
        match reg {
            'a'..='z' | 'A'..='Z' | '0' | '_' | '+' | '"' => {
                self.selected = if reg == '"' { None } else { Some(reg.to_ascii_lowercase()) };
                Ok(())
            }
            _ => Err(format!("Invalid register: {}", reg)),
        }
    }

    /// Get the currently selected register name (for display)
    pub fn selected_register_name(&self) -> String {
        match self.selected {
            None => "\"".to_string(),
            Some(c) => format!("\"{}", c),
        }
    }

    /// Clear the register selection (back to unnamed)
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Check if black hole register is selected
    pub fn is_black_hole(&self) -> bool {
        self.selected == Some('_')
    }

    /// Store content in the appropriate register
    /// - If black hole selected, discards the content
    /// - If yank=true, also updates register 0
    /// - Always updates unnamed register (unless black hole)
    pub fn store(&mut self, content: RegisterContent, is_yank: bool) {
        let reg = self.selected.take();

        // Black hole register - discard everything
        if reg == Some('_') {
            return;
        }

        // Update yank register if this is a yank operation
        if is_yank {
            self.yank_register = Some(content.clone());
        }

        // Store in the appropriate register
        match reg {
            None => {
                // Unnamed register
                self.unnamed = Some(content);
            }
            Some('+') => {
                // System clipboard
                self.unnamed = Some(content.clone());
                let _ = self.write_to_system(&content);
            }
            Some('0') => {
                // Yank register - read only, but store in unnamed
                self.unnamed = Some(content);
            }
            Some(c) if c.is_ascii_lowercase() => {
                // Named register
                self.registers.insert(c, content.clone());
                self.unnamed = Some(content);
            }
            _ => {
                self.unnamed = Some(content);
            }
        }
    }

    /// Retrieve content from the appropriate register
    pub fn retrieve(&mut self) -> Option<RegisterContent> {
        let reg = self.selected.take();

        match reg {
            None => self.unnamed.clone(),
            Some('_') => None, // Black hole is always empty
            Some('+') => {
                // System clipboard
                if let Ok(content) = self.read_from_system() {
                    Some(content)
                } else {
                    None
                }
            }
            Some('0') => self.yank_register.clone(),
            Some(c) if c.is_ascii_lowercase() => {
                self.registers.get(&c).cloned()
            }
            _ => self.unnamed.clone(),
        }
    }

    /// Convenience: yank a single row
    pub fn yank_row(&mut self, row: Vec<String>) {
        self.store(RegisterContent::from_row(row), true);
    }

    /// Convenience: yank multiple rows
    pub fn yank_rows(&mut self, rows: Vec<Vec<String>>) {
        self.store(RegisterContent::from_rows(rows), true);
    }

    /// Convenience: yank a single column
    pub fn yank_col(&mut self, col: Vec<String>) {
        self.store(RegisterContent::from_col(col), true);
    }

    /// Convenience: yank multiple columns
    pub fn yank_cols(&mut self, cols: Vec<Vec<String>>) {
        self.store(RegisterContent::from_cols(cols), true);
    }

    /// Convenience: yank a span
    pub fn yank_span(&mut self, span: Vec<Vec<String>>) {
        self.store(RegisterContent::from_span(span), true);
    }

    /// Store deleted content (goes to unnamed but not yank register)
    pub fn store_deleted(&mut self, content: RegisterContent) {
        self.store(content, false);
    }

    /// Create a paste transaction from the current register
    pub fn paste_as_transaction(
        &mut self,
        cursor_row: usize,
        cursor_col: usize,
        table: &Table,
    ) -> (String, Option<Transaction>) {
        let content = match self.retrieve() {
            Some(c) => c,
            None => return ("Nothing to paste".to_string(), None),
        };

        if content.data.is_empty() {
            return ("Nothing to paste".to_string(), None);
        }

        let rows = content.data.len();
        let cols = content.data.first().map(|r| r.len()).unwrap_or(0);

        // Determine paste position based on anchor
        let (paste_row, paste_col, msg) = match content.anchor {
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
            new_data: content.data,
        };

        (msg, Some(txn))
    }

    /// Write register content to system clipboard
    fn write_to_system(&self, content: &RegisterContent) -> Result<String, String> {
        let tsv: String = content.data
            .iter()
            .map(|row| row.join("\t"))
            .collect::<Vec<_>>()
            .join("\n");

        copy_to_system_clipboard(&tsv)?;

        let rows = content.data.len();
        let cols = content.data.first().map(|r| r.len()).unwrap_or(0);
        Ok(format!("Copied {}x{} to system clipboard", rows, cols))
    }

    /// Read from system clipboard into a RegisterContent
    fn read_from_system(&self) -> Result<RegisterContent, String> {
        let text = paste_from_system_clipboard()?;

        if text.is_empty() {
            return Err("System clipboard is empty".to_string());
        }

        let data: Vec<Vec<String>> = text
            .lines()
            .map(|line| line.split('\t').map(|s| s.to_string()).collect())
            .collect();

        Ok(RegisterContent::new(data, PasteAnchor::Cursor))
    }

    /// Copy current register to system clipboard
    pub fn to_system(&mut self) -> Result<String, String> {
        let content = self.retrieve()
            .ok_or_else(|| "Nothing to copy".to_string())?;
        self.write_to_system(&content)
    }

    /// Paste from system clipboard into unnamed register
    pub fn from_system(&mut self) -> Result<String, String> {
        let content = self.read_from_system()?;
        let rows = content.data.len();
        let cols = content.data.first().map(|r| r.len()).unwrap_or(0);

        self.unnamed = Some(content);
        Ok(format!("Yanked {}x{} from system clipboard", rows, cols))
    }

    /// List non-empty registers (for :registers command)
    pub fn list_registers(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();

        // Unnamed register
        if let Some(ref content) = self.unnamed {
            result.push(("\"\"".to_string(), Self::preview_content(content)));
        }

        // Yank register
        if let Some(ref content) = self.yank_register {
            result.push(("\"0".to_string(), Self::preview_content(content)));
        }

        // Named registers (sorted)
        let mut named: Vec<_> = self.registers.iter().collect();
        named.sort_by_key(|(k, _)| *k);
        for (reg, content) in named {
            result.push((format!("\"{}",reg), Self::preview_content(content)));
        }

        result
    }

    fn preview_content(content: &RegisterContent) -> String {
        let rows = content.data.len();
        let cols = content.data.first().map(|r| r.len()).unwrap_or(0);

        // Show first cell as preview
        let preview = content.data.first()
            .and_then(|r| r.first())
            .map(|s| {
                if s.len() > 20 {
                    format!("{}...", &s[..20])
                } else {
                    s.clone()
                }
            })
            .unwrap_or_default();

        format!("{}x{}: {}", rows, cols, preview)
    }
}

/// Copy text to system clipboard using platform-appropriate method
fn copy_to_system_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        use std::io::Write;

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
        Table::new(
            data.into_iter()
                .map(|row| row.into_iter().map(|s| s.to_string()).collect())
                .collect()
        )
    }

    fn row_manager() -> Rc<RefCell<RowManager>> {
        Rc::new(RefCell::new(RowManager::new()))
    }


    #[test]
    fn test_clipboard_new() {
        let clipboard = Clipboard::new(row_manager());
        assert!(clipboard.unnamed.is_none());
        assert!(clipboard.yank_register.is_none());
        assert!(clipboard.selected.is_none());
    }

    #[test]
    fn test_yank_row_updates_unnamed_and_yank_register() {
        let mut clipboard = Clipboard::new(row_manager());
        clipboard.yank_row(vec!["a".to_string(), "b".to_string()]);

        assert!(clipboard.unnamed.is_some());
        assert!(clipboard.yank_register.is_some());
        assert_eq!(clipboard.unnamed.as_ref().unwrap().data, vec![vec!["a".to_string(), "b".to_string()]]);
    }

    #[test]
    fn test_black_hole_register_discards() {
        let mut clipboard = Clipboard::new(row_manager());
        clipboard.yank_row(vec!["original".to_string()]);

        // Select black hole and yank
        clipboard.select_register('_').unwrap();
        clipboard.yank_row(vec!["discarded".to_string()]);

        // Original should still be in unnamed
        assert_eq!(
            clipboard.unnamed.as_ref().unwrap().data,
            vec![vec!["original".to_string()]]
        );

        // Yank register should still have original
        assert_eq!(
            clipboard.yank_register.as_ref().unwrap().data,
            vec![vec!["original".to_string()]]
        );
    }

    #[test]
    fn test_named_register() {
        let mut clipboard = Clipboard::new(row_manager());

        // Yank to register a
        clipboard.select_register('a').unwrap();
        clipboard.yank_row(vec!["in_a".to_string()]);

        // Yank something else to unnamed
        clipboard.yank_row(vec!["in_unnamed".to_string()]);

        // Retrieve from a
        clipboard.select_register('a').unwrap();
        let content = clipboard.retrieve().unwrap();
        assert_eq!(content.data, vec![vec!["in_a".to_string()]]);
    }

    #[test]
    fn test_yank_register_not_affected_by_delete() {
        let mut clipboard = Clipboard::new(row_manager());

        // Yank something
        clipboard.yank_row(vec!["yanked".to_string()]);

        // Delete something (store_deleted doesn't update yank register)
        clipboard.store_deleted(RegisterContent::from_row(vec!["deleted".to_string()]));

        // Yank register should still have the yank
        assert_eq!(
            clipboard.yank_register.as_ref().unwrap().data,
            vec![vec!["yanked".to_string()]]
        );

        // But unnamed should have the delete
        assert_eq!(
            clipboard.unnamed.as_ref().unwrap().data,
            vec![vec!["deleted".to_string()]]
        );

        // Retrieve from 0 should get the yank
        clipboard.select_register('0').unwrap();
        let content = clipboard.retrieve().unwrap();
        assert_eq!(content.data, vec![vec!["yanked".to_string()]]);
    }

    #[test]
    fn test_paste_anchor_row() {
        let content = RegisterContent::from_row(vec!["a".to_string()]);
        assert_eq!(content.anchor, PasteAnchor::RowStart);
    }

    #[test]
    fn test_paste_anchor_col() {
        let content = RegisterContent::from_col(vec!["a".to_string()]);
        assert_eq!(content.anchor, PasteAnchor::ColStart);
    }

    #[test]
    fn test_paste_anchor_span() {
        let content = RegisterContent::from_span(vec![vec!["a".to_string()]]);
        assert_eq!(content.anchor, PasteAnchor::Cursor);
    }

    #[test]
    fn test_paste_as_transaction_nothing() {
        let mut clipboard = Clipboard::new(row_manager());
        let table = make_table(vec![vec!["a"]]);

        let (msg, txn) = clipboard.paste_as_transaction(0, 0, &table);

        assert_eq!(msg, "Nothing to paste");
        assert!(txn.is_none());
    }

    #[test]
    fn test_paste_as_transaction_row() {
        let mut clipboard = Clipboard::new(row_manager());
        clipboard.yank_row(vec!["x".to_string(), "y".to_string()]);

        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let (msg, txn) = clipboard.paste_as_transaction(0, 0, &table);

        assert_eq!(msg, "1 row(s) pasted");
        assert!(txn.is_some());

        let txn = txn.unwrap();
        let mut table = table;
        txn.apply(&mut table);

        assert_eq!(table.get_row(0).map(|r| r.to_vec()), Some(vec!["x".to_string(), "y".to_string()]));
    }

    #[test]
    fn test_select_invalid_register() {
        let mut clipboard = Clipboard::new(row_manager());
        assert!(clipboard.select_register('!').is_err());
        assert!(clipboard.select_register('1').is_err()); // Only 0 is valid number
    }

    #[test]
    fn test_select_valid_registers() {
        let mut clipboard = Clipboard::new(row_manager());
        assert!(clipboard.select_register('a').is_ok());
        assert!(clipboard.select_register('z').is_ok());
        assert!(clipboard.select_register('A').is_ok()); // Uppercase treated as lowercase
        assert!(clipboard.select_register('0').is_ok());
        assert!(clipboard.select_register('_').is_ok());
        assert!(clipboard.select_register('+').is_ok());
        assert!(clipboard.select_register('"').is_ok());
    }

    #[test]
    fn test_list_registers() {
        let mut clipboard = Clipboard::new(row_manager());
        clipboard.yank_row(vec!["unnamed".to_string()]);

        clipboard.select_register('a').unwrap();
        clipboard.yank_row(vec!["in_a".to_string()]);

        let list = clipboard.list_registers();
        assert!(list.iter().any(|(name, _)| name == "\"\""));
        assert!(list.iter().any(|(name, _)| name == "\"0"));
        assert!(list.iter().any(|(name, _)| name == "\"a"));
    }
}
