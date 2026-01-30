mod app;
mod calculator;
mod clipboard;
mod command;
mod mode;
mod table;
mod transaction;
mod ui;
mod util;
mod fileio;
mod style;

use std::io;
use std::path::PathBuf;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use table::Table;
use fileio::FileIO;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).map(PathBuf::from);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut file_io = FileIO::new(file_path)?;
    let mut table = file_io.load_table()?;

    let mut app = App::new(table, file_io);
    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}
