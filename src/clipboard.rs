use crate::table::{Table, TableView};

pub struct Clipboard {
    pub yanked_row: Option<Vec<String>>,
    pub yanked_col: Option<Vec<String>>,
    pub yanked_span: Option<Vec<Vec<String>>>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self { yanked_row: None, yanked_col: None, yanked_span: None }
    }

    pub fn paste(&mut self, view: &mut TableView, table: &mut Table) -> (String, bool) {
        if let Some(row) = self.yanked_row.clone() {
            view.paste_row( table, row);
            return ("Row pasted".to_string(), true);
        } else if let Some(col) = self.yanked_col.clone() {
            view.paste_col(table, col);
            return ("Column pasted".to_string(), true);
        } else if let Some(span) = self.yanked_span.clone() {
            view.paste_span(table, span);
            return ("Span pasted".to_string(), true);
        } else {
            return ("Nothing to paste".to_string(), false);
        }
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
