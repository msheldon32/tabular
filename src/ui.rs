use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table as RatatuiTable},
    Frame,
};

use crate::app::App;
use crate::mode::Mode;

/// Convert a column index to Excel-style letters (0 -> A, 25 -> Z, 26 -> AA, etc.)
fn col_to_letters(mut col: usize) -> String {
    let mut result = String::new();
    loop {
        result.insert(0, (b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    result
}

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.size());

    render_table(frame, app, chunks[0]);
    render_status_bar(frame, app, chunks[1]);
    render_command_line(frame, app, chunks[2]);
}

fn render_table(frame: &mut Frame, app: &App, area: Rect) {
    let col_count = app.table.col_count();
    if col_count == 0 {
        return;
    }

    // Calculate row number width (for padding)
    let row_num_width = app.table.row_count().to_string().len().max(3);

    // Calculate column widths based on content
    let data_col_widths: Vec<usize> = (0..col_count)
        .map(|col| {
            let content_width = app
                .table
                .cells
                .iter()
                .filter_map(|row| row.get(col))
                .map(|s| s.len())
                .max()
                .unwrap_or(3);
            // Also consider column header width (A, B, ..., AA, etc.)
            let header_width = col_to_letters(col).len();
            content_width.max(header_width).max(3)
        })
        .collect();

    // Build column width constraints: row number column + data columns
    let mut col_widths: Vec<Constraint> = Vec::with_capacity(col_count + 1);
    col_widths.push(Constraint::Length(row_num_width as u16 + 1)); // Row number column
    for w in &data_col_widths {
        col_widths.push(Constraint::Length(*w as u16 + 2));
    }

    // Build header row with column letters
    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let mut header_cells: Vec<Cell> = Vec::with_capacity(col_count + 1);
    header_cells.push(Cell::from("").style(header_style)); // Empty corner cell
    for col in 0..col_count {
        let letter = col_to_letters(col);
        let style = if col == app.table.cursor_col {
            header_style.bg(Color::DarkGray)
        } else {
            header_style
        };
        header_cells.push(Cell::from(letter).style(style));
    }
    let header_row = Row::new(header_cells);

    // Build data rows
    let rows: Vec<Row> = app
        .table
        .cells
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let is_header_row = app.header_mode && row_idx == 0;

            let mut cells: Vec<Cell> = Vec::with_capacity(col_count + 1);

            // Row number cell
            let row_num_style = if row_idx == app.table.cursor_row {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            cells.push(Cell::from(format!("{}", row_idx + 1)).style(row_num_style));

            // Data cells
            for (col_idx, content) in row.iter().enumerate() {
                let is_cursor = row_idx == app.table.cursor_row && col_idx == app.table.cursor_col;

                let style = if is_cursor {
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if is_header_row {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let display_content = if is_cursor && app.mode == Mode::Insert {
                    format!("{}_", &app.edit_buffer)
                } else {
                    content.clone()
                };

                cells.push(Cell::from(display_content).style(style));
            }

            Row::new(cells)
        })
        .collect();

    let table = RatatuiTable::new(rows, col_widths.clone())
        .header(header_row)
        .block(Block::default().borders(Borders::ALL).title("Table"));

    frame.render_widget(table, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.mode {
        Mode::Normal => Style::default().bg(Color::Blue).fg(Color::White),
        Mode::Insert => Style::default().bg(Color::Green).fg(Color::Black),
        Mode::Command => Style::default().bg(Color::Yellow).fg(Color::Black),
    };

    let dirty_indicator = if app.dirty { "[+]" } else { "" };

    let file_name = app
        .file_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "[No File]".to_string());

    let position = format!(
        "{}{} ",
        col_to_letters(app.table.cursor_col),
        app.table.cursor_row + 1
    );

    let status = Line::from(vec![
        Span::styled(
            format!(" {} ", app.mode.display_name()),
            mode_style.add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::raw(file_name),
        Span::raw(" "),
        Span::styled(dirty_indicator, Style::default().fg(Color::Red)),
        Span::raw(" ".repeat(
            area.width
                .saturating_sub(30)
                .saturating_sub(position.len() as u16) as usize,
        )),
        Span::raw(position),
    ]);

    let status_bar = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(status_bar, area);
}

fn render_command_line(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Command => format!(":{}", app.command_buffer),
        _ => {
            if let Some(msg) = &app.message {
                msg.clone()
            } else {
                String::new()
            }
        }
    };

    let command_line = Paragraph::new(content);
    frame.render_widget(command_line, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_to_letters() {
        assert_eq!(col_to_letters(0), "A");
        assert_eq!(col_to_letters(1), "B");
        assert_eq!(col_to_letters(25), "Z");
        assert_eq!(col_to_letters(26), "AA");
        assert_eq!(col_to_letters(27), "AB");
        assert_eq!(col_to_letters(51), "AZ");
        assert_eq!(col_to_letters(52), "BA");
        assert_eq!(col_to_letters(701), "ZZ");
        assert_eq!(col_to_letters(702), "AAA");
    }
}
