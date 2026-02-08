mod app;
mod clipboard;
mod dispatch;
mod fileio;
mod input;
mod mode;
mod plugin;
mod table;
mod transaction;
mod ui;
mod util;
mod numeric;
mod config;
mod string;

use std::io::{self,Write};
use std::panic;
use std::path::PathBuf;
use tracing::{info, error};
use tracing_subscriber;
use tracing_subscriber::fmt::writer::MakeWriter;

use crossterm::{
    execute,
    cursor::MoveToColumn,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use fileio::FileIO;

/// Parse command line arguments
/// Returns (file_path, delimiter)
fn parse_args() -> (Option<PathBuf>, Option<u8>, bool, bool) {
    let args: Vec<String> = std::env::args().collect();
    let mut file_path: Option<PathBuf> = None;
    let mut delimiter: Option<u8> = None;
    let mut fork = false;
    let mut read_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--delimiter" => {
                if i + 1 < args.len() {
                    delimiter = parse_delimiter(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("Error: --delimiter requires an argument");
                    std::process::exit(1);
                }
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-f" | "--fork" => {
                fork = true;
                i += 1;
            }
            "--read-only" => {
                read_only = true;
                i += 1;
            }
            arg if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                std::process::exit(1);
            }
            _ => {
                file_path = Some(PathBuf::from(&args[i]));
                i += 1;
            }
        }
    }

    (file_path, delimiter, fork, read_only)
}

/// Parse a delimiter string into a byte
fn parse_delimiter(s: &str) -> Option<u8> {
    match s.to_lowercase().as_str() {
        "comma" | "," => Some(b','),
        "tab" | "\\t" | "\t" => Some(b'\t'),
        "semicolon" | ";" => Some(b';'),
        "pipe" | "|" => Some(b'|'),
        _ if s.len() == 1 => Some(s.as_bytes()[0]),
        _ => {
            eprintln!("Invalid delimiter: '{}'. Use comma, tab, semicolon, pipe, or a single character.", s);
            std::process::exit(1);
        }
    }
}

/// Handle panics gracefully
fn install_panic_hook() {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();

        if let Some(location) = info.location() {
            error!(
                file = location.file(),
                line = location.line(),
                "panic occured"
            );
        } else {
            error!("panic occured");
        }

        if let Some(s) = info.payload().downcast_ref::<&str>() {
            error!(message = %s);
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            error!(message = %s);
        }

        default_hook(info);
    }));
}

/// A `MakeWriter` for `tracing` that logs to the **main screen** by leaving the alternate screen temporarily.
pub struct MainScreenWriter;

impl<'a> MakeWriter<'a> for MainScreenWriter {
    type Writer = MainScreenWriterHandle;

    fn make_writer(&'a self) -> Self::Writer {
        MainScreenWriterHandle
    }
}

/// A handle that writes to stdout outside the alternate screen
pub struct MainScreenWriterHandle;

impl Write for MainScreenWriterHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Temporarily leave the alternate screen
        execute!(io::stdout(), LeaveAlternateScreen)?;
        println!();
        execute!(io::stdout(), MoveToColumn(0))?;
        let result = io::stdout().write(buf);
        execute!(io::stdout(), MoveToColumn(0))?;
        io::stdout().flush()?; // flush immediately
        // Re-enter the alternate screen
        execute!(io::stdout(), EnterAlternateScreen)?;
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

fn print_help() {
    eprintln!("tabular - A terminal-based CSV editor with vim-like keybindings");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    tabular [OPTIONS] [FILE]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("    -d, --delimiter <DELIM>  Set the field delimiter (comma, tab, semicolon, pipe, or char)");
    eprintln!("    -f, --fork               Fork the file by default");
    eprintln!("    --read-only              Read only mode");
    eprintln!("    -h, --help               Print this help message");
    eprintln!();
    eprintln!("If no delimiter is specified, it will be auto-detected from the file content.");
}

fn main() -> io::Result<()> {
    tracing_subscriber::fmt().with_writer(MainScreenWriter).init();
    info!("Tabular started");

    install_panic_hook();

    let (file_path, delimiter, fork, read_only) = parse_args();

    let file_io = if fork {
        (FileIO::new(file_path, delimiter, read_only)?).fork()
    } else {
        FileIO::new(file_path, delimiter, read_only)?
    };

    let load_result = file_io.load_table().map_err(|e| {error!(error = %e, "Failed to load table"); e})?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Show delimiter info if auto-detected (only when file exists and no explicit delimiter)
    let delimiter_msg = if delimiter.is_none() && file_io.file_path.is_some() {
        Some(format!("Delimiter: {}", file_io.delimiter_name()))
    } else {
        None
    };

    let mut app = App::new(load_result.table, file_io);

    // Show any warnings from loading (e.g., "New file", "Padded rows")
    let mut messages: Vec<String> = load_result.warnings;
    if let Some(msg) = delimiter_msg {
        messages.push(msg);
    }
    if !messages.is_empty() {
        app.message = Some(messages.join("; "));
    }

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}
