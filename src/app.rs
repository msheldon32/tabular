use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, poll, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::calculator::Calculator;
use crate::command::Command;
use crate::mode::Mode;
use crate::table::{Table, TableView};
use crate::ui;
use crate::clipboard::Clipboard;
use crate::util::{CellRef, parse_cell_ref, parse_range};

pub struct App {
    pub table: Table,
    pub view: TableView,
    pub clipboard: Clipboard,
    pub mode: Mode,
    pub command_buffer: String,
    pub edit_buffer: String,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub message: Option<String>,
    pub should_quit: bool,
    pub pending_key: Option<char>,
    pub header_mode: bool,
}

impl App {
    pub fn new(file_path: Option<PathBuf>) -> io::Result<Self> {
        let table = if let Some(ref path) = file_path {
            Table::load_csv(path)?
        } else {
            Table::new()
        };

        let mut view = TableView::new();
        view.update_col_widths(&table);

        let clipboard = Clipboard::new();

        Ok(Self {
            table,
            view,
            clipboard,
            mode: Mode::Normal,
            command_buffer: String::new(),
            edit_buffer: String::new(),
            file_path,
            dirty: false,
            message: None,
            should_quit: false,
            pending_key: None,
            header_mode: true,
        })
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        // Initial column width calculation
        self.view.update_col_widths(&self.table);

        while !self.should_quit {
            terminal.draw(|f| ui::render(f, self))?;

            if poll(Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    self.message = None;
                    self.handle_key(key);
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Command => self.handle_command_mode(key),
            Mode::VisualRow => self.handle_visual_row_mode(key),
            Mode::VisualCol => self.handle_visual_col_mode(key),
            Mode::Visual => self.handle_visual_mode(key)
        }
    }

    fn handle_navigation(&mut self, key: KeyEvent) {
        if let Some(pending) = self.pending_key.take() {
            match (pending, key.code) {
                ('g', KeyCode::Char('g')) => {
                    self.view.move_to_top();
                }
                _ => {
                    // Invalid sequence, ignore
                }
            }
        }
        match key.code {
            // Navigation
            KeyCode::Char('h') | KeyCode::Left => self.view.move_left(),
            KeyCode::Char('j') | KeyCode::Down => self.view.move_down(&self.table),
            KeyCode::Char('k') | KeyCode::Up => self.view.move_up(),
            KeyCode::Char('l') | KeyCode::Right => self.view.move_right(&self.table),

            // Jump navigation
            KeyCode::Char('g') => {
                self.pending_key = Some('g');
            }
            KeyCode::Char('G') => {
                self.view.move_to_bottom(&self.table);
            }
            KeyCode::Char('0') => {
                self.view.move_to_first_col();
            }
            KeyCode::Char('^') => {
                self.view.move_to_first_col();
            }
            KeyCode::Char('$') => {
                self.view.move_to_last_col(&self.table);
            }

            // Page navigation
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.half_page_down(&self.table);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.half_page_up();
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.page_down(&self.table);
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.view.page_up();
            }
            _ => {}
        }
    }

    fn handle_visual_mode(&mut self, key: KeyEvent) {
        let is_escape = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL));

        if is_escape {
            self.mode = Mode::Normal;
            self.view.update_col_widths(&self.table);
            return;
        }

        self.handle_navigation(key);

        match key.code {
            KeyCode::Char('y') => {
                if let Some(span) = self.view.yank_span(&mut self.table) {
                    self.clipboard.yank_span(span)
                }

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            KeyCode::Char('x') => {
                self.view.clear_span(&mut self.table);
                self.dirty = true;

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            _ => {}
        }
    }

    fn handle_visual_row_mode(&mut self, key: KeyEvent) {
        let is_escape = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL));

        if is_escape {
            self.mode = Mode::Normal;
            self.view.update_col_widths(&self.table);
            return;
        }

        self.handle_navigation(key);

        match key.code {
            KeyCode::Char('y') => {
                if let Some(span) = self.view.yank_row(&mut self.table) {
                    self.clipboard.yank_row(span)
                }

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            KeyCode::Char('x') => {
                self.view.clear_row_span(&mut self.table);
                self.dirty = true;

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            // dragging
            KeyCode::Char('q') => {
                self.view.drag_down(&mut self.table, true);
                self.dirty = true;

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            _ => {}
        }
    }

    fn handle_visual_col_mode(&mut self, key: KeyEvent) {
        let is_escape = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL));

        if is_escape {
            self.mode = Mode::Normal;
            self.view.update_col_widths(&self.table);
            return;
        }

        self.handle_navigation(key);

        match key.code {
            KeyCode::Char('y') => {
                if let Some(span) = self.view.yank_col(&mut self.table) {
                    self.clipboard.yank_col(span)
                }

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            KeyCode::Char('x') => {
                self.view.clear_col_span(&mut self.table);
                self.dirty = true;

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            // dragging
            KeyCode::Char('q') => {
                self.view.drag_right(&mut self.table, true);
                self.dirty = true;

                self.mode = Mode::Normal;
                self.view.update_col_widths(&mut self.table);
                return;
            }

            _ => {}
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        // Handle pending key sequences (dr, dc, yr, yc, gg)
        if let Some(pending) = self.pending_key.take() {
            match (pending, key.code) {
                ('d', KeyCode::Char('r')) => {
                    if let Some(row) = self.view.delete_row(&mut self.table) {
                        self.clipboard.yank_row(row);
                        self.dirty = true;
                        self.message = Some("Row deleted".to_string());
                        return;
                    }
                }
                ('d', KeyCode::Char('c')) => {
                    if let Some(col) = self.view.delete_col(&mut self.table) {
                        self.clipboard.yank_col(col);
                        self.dirty = true;
                        self.message = Some("Column deleted".to_string());
                        return;
                    }
                }
                ('y', KeyCode::Char('r')) => {
                    if let Some(row) = self.view.yank_row(&self.table) {
                        self.clipboard.yank_row(row);
                        self.message = Some("Row yanked".to_string());
                        return;
                    }
                }
                ('y', KeyCode::Char('c')) => {
                    if let Some(col) = self.view.yank_col(&self.table) {
                        self.clipboard.yank_col(col);
                        self.message = Some("Column yanked".to_string());
                        return;
                    }
                }
                ('g', KeyCode::Char('g')) => {
                    self.view.move_to_top();
                    return;
                }
                _ => {
                    // Invalid sequence, ignore
                }
            }
        }

        self.handle_navigation(key);

        match key.code {
            // Insert mode
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                self.edit_buffer = self.view.current_cell(&self.table).clone();
            }

            // Visual row mode
            KeyCode::Char('V') => {
                self.mode = Mode::VisualRow;
                self.view.set_support();
            }

            // Visual column mode
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::VisualCol;
                self.view.set_support();

                return;
            }

            // Vanilla Visual mode
            KeyCode::Char('v') => {
                self.mode = Mode::Visual;
                self.view.set_support();
            }


            // Command mode
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
            }

            // Quit
            KeyCode::Char('q') => {
                if self.dirty {
                    self.message = Some("Unsaved changes! Use :q! to force quit".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }

            // Row operations
            KeyCode::Char('o') => {
                self.view.insert_row_below(&mut self.table);
                self.dirty = true;
                self.message = Some("Row added".to_string());
            }
            KeyCode::Char('O') => {
                self.view.insert_row_above(&mut self.table);
                self.dirty = true;
                self.message = Some("Row added".to_string());
            }
            KeyCode::Char('d') => {
                self.pending_key = Some('d');
            }
            KeyCode::Char('y') => {
                self.pending_key = Some('y');
            }
            KeyCode::Char('p') => {
                let (message, succeeded) = self.clipboard.paste(&mut self.view, &mut self.table);

                if succeeded {
                    self.dirty = true;
                }
                self.message = Some(message);
            }

            // Column operations
            KeyCode::Char('A') => {
                self.view.insert_col_after(&mut self.table);
                self.dirty = true;
                self.message = Some("Column added".to_string());
            }
            KeyCode::Char('X') => {
                self.view.delete_col(&mut self.table);
                self.dirty = true;
                self.message = Some("Column deleted".to_string());
            }

            // Cell operations
            KeyCode::Char('x') => {
                *self.view.current_cell_mut(&mut self.table) = String::new();
                self.dirty = true;
            }

            _ => {}
        }
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) {
        // Ctrl+[ is equivalent to Escape and often faster in terminals
        let is_escape = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL));

        if is_escape {
            *self.view.current_cell_mut(&mut self.table) = self.edit_buffer.clone();
            self.dirty = true;
            self.mode = Mode::Normal;
            self.view.update_col_widths(&self.table);
            return;
        }

        match key.code {
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
            }
            KeyCode::Enter => {
                *self.view.current_cell_mut(&mut self.table) = self.edit_buffer.clone();
                self.dirty = true;
                self.mode = Mode::Normal;
                self.view.update_col_widths(&self.table);
            }
            _ => {}
        }

        self.view.expand_column(self.edit_buffer.len());
    }

    fn handle_command_mode(&mut self, key: KeyEvent) {
        // Ctrl+[ is equivalent to Escape
        let is_escape = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL));

        if is_escape {
            self.mode = Mode::Normal;
            self.command_buffer.clear();
            return;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(cmd) = Command::parse(&self.command_buffer) {
                    self.execute_command(cmd);
                }
                self.command_buffer.clear();
                if self.mode == Mode::Command {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::Write => {
                if let Some(ref path) = self.file_path {
                    match self.table.save_csv(path) {
                        Ok(()) => {
                            self.dirty = false;
                            self.message = Some(format!("Saved to {}", path.display()));
                        }
                        Err(e) => {
                            self.message = Some(format!("Error saving: {}", e));
                        }
                    }
                } else {
                    self.message = Some("No file path specified".to_string());
                }
            }
            Command::Quit => {
                if self.dirty {
                    self.message = Some("Unsaved changes! Use :q! to force quit".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            Command::ForceQuit => {
                self.should_quit = true;
            }
            Command::WriteQuit => {
                if let Some(ref path) = self.file_path {
                    match self.table.save_csv(path) {
                        Ok(()) => {
                            self.should_quit = true;
                        }
                        Err(e) => {
                            self.message = Some(format!("Error saving: {}", e));
                        }
                    }
                } else {
                    self.message = Some("No file path specified".to_string());
                }
            }
            Command::AddColumn => {
                self.view.insert_col_after(&mut self.table);
                self.dirty = true;
                self.message = Some("Column added".to_string());
            }
            Command::DeleteColumn => {
                self.view.delete_col(&mut self.table);
                self.dirty = true;
                self.message = Some("Column deleted".to_string());
            }
            Command::ToggleHeader => {
                self.header_mode = !self.header_mode;
                self.message = Some(format!(
                    "Header mode {}",
                    if self.header_mode { "on" } else { "off" }
                ));
            }
            Command::Calc => {
                let calc = Calculator::new(&self.table);
                match calc.evaluate_all() {
                    Ok(updates) => {
                        let count = updates.len();
                        for (row, col, value) in updates {
                            self.table.set_cell(row, col, value);
                        }
                        if count > 0 {
                            self.dirty = true;
                            self.view.update_col_widths(&self.table);
                            self.message = Some(format!("Evaluated {} formula(s)", count));
                        } else {
                            self.message = Some("No formulas found".to_string());
                        }
                    }
                    Err(e) => {
                        self.message = Some(format!("{}", e));
                    }
                }
            }
            Command::NavigateRow(row) => {
                self.view.cursor_row = row;
            }
            Command::NavigateCell(cell) => {
                self.view.cursor_row = cell.row;
                self.view.cursor_col = cell.col;
            }
            Command::Unknown(s) => {
                self.message = Some(format!("Unknown command: {}", s));
            }
        }
    }
}
