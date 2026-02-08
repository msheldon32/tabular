use super::transaction::Transaction;

/// Manages undo/redo history
#[derive(Debug, Default)]
pub struct History {
    undo_stack: Vec<Transaction>,
    redo_stack: Vec<Transaction>,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Record a transaction (clears redo stack)
    pub fn record(&mut self, txn: Transaction) {
        if matches!(txn, Transaction::Undo | Transaction::Redo) {
            // cannot undo/redo these for obvious reasons
            return;
        }
        self.undo_stack.push(txn);
        self.redo_stack.clear();
    }

    /// Undo the last transaction, returns the inverse for application
    pub fn undo(&mut self) -> Option<Transaction> {
        self.undo_stack.pop().map(|txn| {
            let inverse = txn.inverse();
            self.redo_stack.push(txn);
            inverse
        })
    }

    /// Redo the last undone transaction
    pub fn redo(&mut self) -> Option<Transaction> {
        self.redo_stack.pop().map(|txn| {
            self.undo_stack.push(txn.clone());
            txn
        })
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Peek at the next undo transaction without removing it
    pub fn peek_undo(&self) -> Option<&Transaction> {
        self.undo_stack.last()
    }

    /// Peek at the next redo transaction without removing it
    pub fn peek_redo(&self) -> Option<&Transaction> {
        self.redo_stack.last()
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
