mod app;
mod calculator;
mod clipboard;
mod command;
mod fileio;
mod format;
mod input;
mod mode;
mod style;
mod table;
mod transaction;
mod ui;
mod util;

use std::io;
use std::path::PathBuf;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use fileio::FileIO;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).map(PathBuf::from);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let file_io = FileIO::new(file_path)?;
    let load_result = file_io.load_table()?;

    let mut app = App::new(load_result.table, file_io);

    // Show any warnings from loading (e.g., "New file", "Padded rows")
    if !load_result.warnings.is_empty() {
        app.message = Some(load_result.warnings.join("; "));
    }

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}
