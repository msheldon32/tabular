use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::table::table::Table;
use crate::table::tableview::TableView;
use crate::transaction::Transaction;

/// Result of handling a key event
#[allow(dead_code)]
pub enum KeyResult {
    /// Continue in current mode
    Continue,
    /// Switch to a different mode
    SwitchMode(crate::mode::Mode),
    /// Execute a transaction
    Execute(Transaction),
    /// Execute a transaction and return to normal mode
    ExecuteAndFinish(Transaction),
    /// Return to normal mode
    Finish,
    /// Show a message
    Message(String),
    /// Quit the application
    Quit,
    /// Force quit
    ForceQuit,
}

/// Check for escape key (Esc or Ctrl+[)
pub fn is_escape(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL))
}

/// Actions resulting from key sequences
#[derive(Clone, Debug, PartialEq)]
pub enum SequenceAction {
    MoveToTop,   // gg
    DeleteRow,   // dr
    DeleteCol,   // dc
    YankRow,     // yr
    YankCol,     // yc
    Yank,        // yy (visual mode)
    Delete,      // dd (visual mode)
    // Motion actions (can have count)
    MoveDown,    // j
    MoveUp,      // k
    MoveLeft,    // h
    MoveRight,   // l
    // Format actions (visual mode)
    FormatDefault,         // ff - reset to default number format
    FormatCommas,          // f, - add comma separators
    FormatCurrency,        // f$
    FormatScientific,      // fe
    FormatPercentage,      // f%
    // Register selection
    SelectRegister(char),  // "x
}

impl SequenceAction {
    #[allow(dead_code)]
    pub fn is_navigation(&self) -> bool {
        matches!(self, 
                 SequenceAction::MoveToTop |
                 SequenceAction::MoveDown  |
                 SequenceAction::MoveLeft  |
                 SequenceAction::MoveRight
                 )
    }
}

/// Result of processing a key through the buffer
pub enum KeyBufferResult {
    /// A sequence matched, execute this action with optional count
    Action(SequenceAction, usize),
    /// Waiting for more keys (buffer is a valid prefix)
    Pending,
    /// No sequence matched, process this key normally (with optional count)
    Fallthrough(KeyEvent, usize),
}

/// Buffer for accumulating multi-key sequences with optional count prefix
pub struct KeyBuffer {
    keys: Vec<char>,
    count: Option<usize>,
    last_key_time: Instant,
    timeout: Duration,
}

impl KeyBuffer {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            count: None,
            last_key_time: Instant::now(),
            timeout: Duration::from_millis(1000),
        }
    }

    /// Process a key event, returning what action to take
    pub fn process(&mut self, key: KeyEvent) -> KeyBufferResult {
        // Clear buffer if too much time has passed since last key
        if self.last_key_time.elapsed() > self.timeout {
            self.keys.clear();
            self.count = None;
        }

        // Only buffer character keys (no modifiers except shift)
        let c = match key.code {
            KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => c,
            _ => {
                // Non-char key breaks any sequence
                let count = self.take_count();
                self.keys.clear();
                return KeyBufferResult::Fallthrough(key, count);
            }
        };

        self.last_key_time = Instant::now();

        // Accumulate digits as count prefix (but not '0' at start - that's a motion)
        if c.is_ascii_digit() && (self.count.is_some() || c != '0' || !self.keys.is_empty()) {
            if self.keys.is_empty() {
                // Still in count prefix phase
                let digit = c.to_digit(10).unwrap() as usize;
                self.count = Some(self.count.unwrap_or(0) * 10 + digit);
                return KeyBufferResult::Pending;
            }
        }

        self.keys.push(c);

        // Try to match a complete sequence
        if let Some(action) = self.match_sequence() {
            let count = self.take_count();
            self.keys.clear();
            return KeyBufferResult::Action(action, count);
        }

        // Check if current buffer could be a prefix of any sequence
        if self.is_valid_prefix() {
            return KeyBufferResult::Pending;
        }

        // No match and not a valid prefix - clear and fall through
        let count = self.take_count();
        self.keys.clear();
        KeyBufferResult::Fallthrough(key, count)
    }

    /// Clear the buffer (e.g., on mode change)
    pub fn clear(&mut self) {
        self.keys.clear();
        self.count = None;
    }

    /// Get the current buffer contents for display
    pub fn display(&self) -> String {
        let mut result = String::new();
        if let Some(count) = self.count {
            result.push_str(&count.to_string());
        }
        for c in &self.keys {
            result.push(*c);
        }
        result
    }

    fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    fn match_sequence(&self) -> Option<SequenceAction> {
        match self.keys.as_slice() {
            ['g', 'g'] => Some(SequenceAction::MoveToTop),
            ['d', 'r'] => Some(SequenceAction::DeleteRow),
            ['d', 'c'] => Some(SequenceAction::DeleteCol),
            ['d', 'd'] => Some(SequenceAction::Delete),
            ['y', 'r'] => Some(SequenceAction::YankRow),
            ['y', 'c'] => Some(SequenceAction::YankCol),
            ['y', 'y'] => Some(SequenceAction::Yank),
            ['j'] => Some(SequenceAction::MoveDown),
            ['k'] => Some(SequenceAction::MoveUp),
            ['h'] => Some(SequenceAction::MoveLeft),
            ['l'] => Some(SequenceAction::MoveRight),
            // Format sequences
            ['f', 'f'] => Some(SequenceAction::FormatDefault),
            ['f', ','] => Some(SequenceAction::FormatCommas),
            ['f', '$'] => Some(SequenceAction::FormatCurrency),
            ['f', 'e'] => Some(SequenceAction::FormatScientific),
            ['f', '%'] => Some(SequenceAction::FormatPercentage),
            // Register selection: "x where x is a valid register
            ['"', c] if matches!(c, 'a'..='z' | 'A'..='Z' | '0' | '_' | '+' | '"') => {
                Some(SequenceAction::SelectRegister(*c))
            }
            _ => None,
        }
    }

    fn is_valid_prefix(&self) -> bool {
        matches!(self.keys.as_slice(), ['g'] | ['d'] | ['y'] | ['f'] | ['"'])
    }
}

/// Navigation handler shared across modes
pub struct NavigationHandler;

impl NavigationHandler {
    pub fn new() -> Self {
        Self
    }

    /// Handle sequence actions
    pub fn handle_sequence(&self, action: SequenceAction, count: usize, view: &mut TableView, table: &Table) {
        match action {
            SequenceAction::MoveToTop => {
                view.move_to_top();
            }
            SequenceAction::MoveDown => {
                view.move_down_n(count, &table);
            }
            SequenceAction::MoveUp => {
                view.move_up_n(count);
            }
            SequenceAction::MoveLeft => {
                view.move_left_n(count);
            }
            SequenceAction::MoveRight => {
                view.move_right_n(count, &table);
            }

            _ => {}
        }
    }

    /// Handle navigation keys, returns true if the key was handled
    pub fn handle(&self, key: KeyEvent, view: &mut TableView, table: &Table) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            // Jump navigation: Ctrl+Arrow or Ctrl+hjkl
            KeyCode::Left if ctrl => { view.jump_left(table); true }
            KeyCode::Right if ctrl => { view.jump_right(table); true }
            KeyCode::Up if ctrl => { view.jump_up(table); true }
            KeyCode::Down if ctrl => { view.jump_down(table); true }
            KeyCode::Char('h') if ctrl => { view.jump_left(table); true }
            KeyCode::Char('j') if ctrl => { view.jump_down(table); true }
            KeyCode::Char('k') if ctrl => { view.jump_up(table); true }
            KeyCode::Char('l') if ctrl => { view.jump_right(table); true }

            // Regular navigation
            KeyCode::Char('h') | KeyCode::Left => { view.move_left(); true }
            KeyCode::Char('j') | KeyCode::Down => { view.move_down(table); true }
            KeyCode::Char('k') | KeyCode::Up => { view.move_up(); true }
            KeyCode::Char('l') | KeyCode::Right => { view.move_right(table); true }
            KeyCode::Char('G') => { view.move_to_bottom(table); true }
            KeyCode::Char('0') | KeyCode::Char('^') => { view.move_to_first_col(); true }
            KeyCode::Char('$') => { view.move_to_last_col(table); true }
            KeyCode::Char('d') if ctrl => {
                view.half_page_down(table); true
            }
            KeyCode::Char('u') if ctrl => {
                view.half_page_up(); true
            }
            KeyCode::Char('f') if ctrl => {
                view.page_down(table); true
            }
            KeyCode::Char('b') if ctrl => {
                view.page_up(); true
            }
            _ => false
        }
    }
}
