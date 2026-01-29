use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, poll, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::command::Command;
use crate::mode::Mode;
use crate::table::Table;
use crate::ui;

pub struct App {
    pub table: Table,
    pub mode: Mode,
    pub command_buffer: String,
    pub edit_buffer: String,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub message: Option<String>,
    pub should_quit: bool,
    pub yanked_row: Option<Vec<String>>,
    pub yanked_col: Option<Vec<String>>,
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

        Ok(Self {
            table,
            mode: Mode::Normal,
            command_buffer: String::new(),
            edit_buffer: String::new(),
            file_path,
            dirty: false,
            message: None,
            should_quit: false,
            yanked_row: None,
            yanked_col: None,
            pending_key: None,
            header_mode: true,
        })
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
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
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        // Handle pending key sequences (dd, yy)
        if let Some(pending) = self.pending_key.take() {
            match (pending, key.code) {
                ('d', KeyCode::Char('r')) => {
                    if let Some(row) = self.table.delete_row() {
                        self.yanked_row = Some(row);
                        self.yanked_col = None;
                        self.dirty = true;
                        self.message = Some("Row deleted".to_string());
                    }
                }
                ('d', KeyCode::Char('r')) => {
                    if let Some(col) = self.table.delete_column() {
                        self.yanked_row = None;
                        self.yanked_col = Some(col);
                        self.dirty = true;
                        self.message = Some("Column deleted".to_string());
                    }
                }
                ('y', KeyCode::Char('r')) => {
                    self.yanked_row = Some(self.table.yank_row());
                    self.yanked_col = None;
                    self.message = Some("Row yanked".to_string());
                }
                ('y', KeyCode::Char('c')) => {
                    self.yanked_col = Some(self.table.yank_column());
                    self.yanked_row = None;
                    self.message = Some("Column yanked".to_string());
                }
                _ => {
                    // Invalid sequence, ignore
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => self.table.move_left(),
            KeyCode::Char('j') | KeyCode::Down => self.table.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.table.move_up(),
            KeyCode::Char('l') | KeyCode::Right => self.table.move_right(),
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                self.edit_buffer = self.table.current_cell().clone();
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
            }
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
                self.table.insert_row_below();
                self.dirty = true;
                self.message = Some("Row added".to_string());
            }
            KeyCode::Char('O') => {
                self.table.insert_row_above();
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
                if let Some(row) = self.yanked_row.clone() {
                    self.table.paste_row_below(row);
                    self.dirty = true;
                    self.message = Some("Row pasted".to_string());
                } else if let Some(col) = self.yanked_col.clone() {
                    self.table.paste_column_after(col);
                    self.dirty = true;
                    self.message = Some("Column pasted".to_string());
                } else {
                    self.message = Some("Nothing to paste".to_string());
                }
            }
            // Column operations
            KeyCode::Char('A') => {
                self.table.add_column_after();
                self.dirty = true;
                self.message = Some("Column added".to_string());
            }
            KeyCode::Char('X') => {
                self.table.delete_column();
                self.dirty = true;
                self.message = Some("Column deleted".to_string());
            }
            // Cell operations
            KeyCode::Char('x') => {
                *self.table.current_cell_mut() = String::new();
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
            *self.table.current_cell_mut() = self.edit_buffer.clone();
            self.dirty = true;
            self.mode = Mode::Normal;
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
                *self.table.current_cell_mut() = self.edit_buffer.clone();
                self.dirty = true;
                self.mode = Mode::Normal;
            }
            _ => {}
        }
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
                let cmd = Command::parse(&self.command_buffer);
                self.execute_command(cmd);
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
                self.table.add_column_after();
                self.dirty = true;
                self.message = Some("Column added".to_string());
            }
            Command::DeleteColumn => {
                self.table.delete_column();
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
            Command::Unknown(s) => {
                self.message = Some(format!("Unknown command: {}", s));
            }
        }
    }
}
