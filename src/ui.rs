use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table as RatatuiTable},
    Frame,
};

use std::rc::Rc;
use std::cell::RefCell;

use crate::app::App;
use crate::format::format_display;
use crate::mode::Mode;
use crate::util::letters_from_col;
use crate::rowmanager::RowManager;

pub fn render(frame: &mut Frame, app: &mut App, row_manager: Rc<RefCell<RowManager>>) {
    // Apply background color if set
    if let Some(bg_color) = app.style.background() {
        let bg_style = Style::default().bg(bg_color);
        frame.render_widget(Clear, frame.size());
        frame.render_widget(Block::default().style(bg_style), frame.size());
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.size());

    render_table(frame, app, chunks[0], row_manager);
    render_status_bar(frame, app, chunks[1]);
    render_command_line(frame, app, chunks[2]);
}

fn render_table(frame: &mut Frame, app: &mut App, area: Rect, row_manager: Rc<RefCell<RowManager>>) {
    let col_count = app.table.col_count();
    let row_count = app.table.row_count();
    if col_count == 0 || row_count == 0 {
        return;
    }

    // Calculate available space for data (accounting for borders and row numbers)
    let row_num_width = row_count.to_string().len().max(3);
    let available_width = area.width.saturating_sub(4 + row_num_width as u16); // borders + row nums
    let available_height = area.height.saturating_sub(3); // borders + header

    // Update visible rows/cols in view
    let visible_rows = available_height as usize;

    // Calculate how many columns fit in available width
    let mut total_width = 0u16;
    let mut visible_cols = 0usize;
    for col in app.view.viewport_col..col_count {
        let col_width = app.table.col_widths().get(col).copied().unwrap_or(3);
        let cell_width = col_width as u16 + 2; // padding
        if total_width + cell_width > available_width && visible_cols > 0 {
            break;
        }
        total_width += cell_width;
        visible_cols += 1;
    }
    app.view.viewport_width = visible_cols;

    // Ensure cursor is visible
    app.view.scroll_to_cursor();

    // Calculate column widths for visible columns
    let mut col_widths: Vec<Constraint> = Vec::with_capacity(visible_cols + 1);
    col_widths.push(Constraint::Length(row_num_width as u16 + 1)); // Row number column

    let end_col = (app.view.viewport_col + visible_cols).min(col_count);
    for col in app.view.viewport_col..end_col {
        let w = app.table.col_widths().get(col).copied().unwrap_or(3);
        let header_w = letters_from_col(col).len();
        col_widths.push(Constraint::Length(w.max(header_w) as u16 + 2));
    }

    // Build header row with column letters
    let header_style = app.style.header_col();

    let mut header_cells: Vec<Cell> = Vec::with_capacity(visible_cols + 1);
    header_cells.push(Cell::from("").style(header_style)); // Empty corner cell

    for col in app.view.viewport_col..end_col {
        let letter = letters_from_col(col);
        let style = if col == app.view.cursor_col {
            app.style.row_number_cursor()
        } else {
            header_style
        };
        header_cells.push(Cell::from(letter).style(style));
    }
    let header_row = Row::new(header_cells);

    // Selected row indices
    let selected_indices: Box<dyn Iterator<Item = usize>> = if app.header_mode && app.view.viewport_row > 0 {
        // this hangs without the row count end, guess iters aren't that lazy after all
        Box::new((0..1).chain(app.view.viewport_row..app.table.row_count()).filter(|&i| row_manager.borrow().is_row_live(i)).take(visible_rows))
    } else {
        Box::new((app.view.viewport_row..app.table.row_count()).filter(|&i| row_manager.borrow().is_row_live(i)).take(visible_rows))
    };

    let mut end_row = 0;

    // Build data rows (only visible ones)
    let rows: Vec<Row> = selected_indices
        .map(|row_idx| {
            end_row = row_idx;

            let is_header_row = app.header_mode && row_idx == 0;

            let mut cells: Vec<Cell> = Vec::with_capacity(visible_cols + 1);

            // Row number cell
            let row_num_style = if row_idx == app.view.cursor_row {
                app.style.row_number_cursor()
            } else {
                app.style.row_number()
            };
            cells.push(Cell::from(format!("{}", row_idx + 1)).style(row_num_style));

            // Data cells (only visible columns)
            for col_idx in app.view.viewport_col..end_col {
                let raw_content = app.table.get_cell(row_idx, col_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                // Apply precision formatting for display
                let content = format_display(raw_content, app.precision);

                let is_cursor = row_idx == app.view.cursor_row && col_idx == app.view.cursor_col;
                let is_selected = matches!(app.mode, Mode::Visual | Mode::VisualCol | Mode::VisualRow)
                    && app.view.is_selected(row_idx, col_idx, app.mode);

                // Check if this cell matches the search pattern
                let is_search_match = app.search_pattern()
                    .map(|p| content.to_lowercase().contains(&p.to_lowercase()))
                    .unwrap_or(false);

                let style = if is_cursor {
                    app.style.cell_cursor()
                } else if is_selected {
                    app.style.cell_selection()
                } else if is_search_match {
                    app.style.cell_match()
                } else if is_header_row {
                    app.style.header_row()
                } else {
                    app.style.cell()
                };

                let display_content = if is_cursor && app.mode == Mode::Insert {
                    let buf = app.edit_buffer();
                    let cursor_char = app.edit_cursor(); // Character index
                    let char_count = crate::util::char_count(buf);

                    // Get byte indices for slicing
                    let cursor_byte = crate::util::byte_index_of_char(buf, cursor_char);
                    let next_byte = crate::util::byte_index_of_char(buf, cursor_char + 1);

                    let (before, cursor_char_str, after) = if cursor_char >= char_count {
                        // Cursor at end - show space as cursor
                        (buf.to_string(), " ".to_string(), String::new())
                    } else {
                        (
                            buf[..cursor_byte].to_string(),
                            buf[cursor_byte..next_byte].to_string(),
                            buf[next_byte..].to_string(),
                        )
                    };

                    let spans = vec![
                        Span::raw(before),
                        Span::styled(cursor_char_str, Style::default().add_modifier(Modifier::UNDERLINED)),
                        Span::raw(after),
                    ];
                    Line::from(spans)
                } else {
                    Line::from(vec![Span::raw(content.to_string())])
                };

                cells.push(Cell::from(display_content).style(style));
            }

            Row::new(cells)
        })
        .collect();
    
    // update viewport
    app.view.viewport_height = visible_rows-1;

    // Build title with scroll indicator
    let title = if app.view.viewport_row > 0 || app.view.viewport_col > 0 {
        format!(
            "Table [{}-{}/{} rows, {}-{}/{} cols]",
            app.view.viewport_row + 1,
            end_row,
            row_count,
            letters_from_col(app.view.viewport_col),
            letters_from_col(end_col.saturating_sub(1)),
            letters_from_col(col_count.saturating_sub(1))
        )
    } else {
        format!("Table [{} rows, {} cols]", row_count, col_count)
    };

    let mut table_block = Block::default().borders(Borders::ALL).title(title);
    if let Some(bg_color) = app.style.background() {
        table_block = table_block.style(Style::default().bg(bg_color));
    }

    let table = RatatuiTable::new(rows, col_widths)
        .header(header_row)
        .block(table_block);

    frame.render_widget(table, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_style = app.style.status_mode(&app.mode);

    let dirty_indicator = if app.dirty { "[+]" } else { "" };

    let file_name = app
        .file_io
        .file_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "[No File]".to_string());

    let position = format!(
        "{}{} ",
        letters_from_col(app.view.cursor_col),
        app.view.cursor_row + 1
    );

    let key_buffer = app.key_buffer_display();
    let key_buffer_display = if key_buffer.is_empty() {
        String::new()
    } else {
        format!("{} ", key_buffer)
    };

    let right_side_len = position.len() + key_buffer_display.len();

    let status = Line::from(vec![
        Span::styled(
            format!(" {} ", app.mode.display_name()),
            mode_style,
        ),
        Span::raw(" "),
        Span::raw(file_name),
        Span::raw(" "),
        Span::styled(dirty_indicator, app.style.message_error()),
        Span::raw(" ".repeat(
            area.width
                .saturating_sub(30)
                .saturating_sub(right_side_len as u16) as usize,
        )),
        Span::styled(key_buffer_display, app.style.status_mode(&app.mode)),
        Span::raw(position),
    ]);

    let status_bar = Paragraph::new(status).style(app.style.status_bar());

    frame.render_widget(status_bar, area);
}

fn render_command_line(frame: &mut Frame, app: &App, area: Rect) {
    let (content, style) = match app.mode {
        Mode::Command => {
            let line = Line::from(vec![
                Span::styled(":", app.style.command_prompt()),
                Span::styled(app.command_buffer(), app.style.command_line()),
            ]);
            (line, app.style.command_line())
        }
        Mode::Search => {
            let line = Line::from(vec![
                Span::styled("/", app.style.command_prompt()),
                Span::styled(app.search_buffer(), app.style.command_line()),
            ]);
            (line, app.style.command_line())
        }
        _ => {
            // Check for active progress first
            if let Some((ref op_name, ref progress)) = app.progress {
                let pct = progress.percent();
                let bar_width = 20usize;
                let filled = (pct * bar_width) / 100;
                let empty = bar_width.saturating_sub(filled);
                let bar = format!(
                    "{}: [{}{}] {}%",
                    op_name,
                    "█".repeat(filled),
                    "░".repeat(empty),
                    pct
                );
                let line = Line::from(bar);
                (line, app.style.message_info())
            } else {
                let msg = app.message.as_deref().unwrap_or("");
                let line = Line::from(msg);
                (line, app.style.message_info())
            }
        }
    };

    let command_line = Paragraph::new(content).style(style);
    frame.render_widget(command_line, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_letters_from_col() {
        assert_eq!(letters_from_col(0), "A");
        assert_eq!(letters_from_col(1), "B");
        assert_eq!(letters_from_col(25), "Z");
        assert_eq!(letters_from_col(26), "AA");
        assert_eq!(letters_from_col(27), "AB");
        assert_eq!(letters_from_col(51), "AZ");
        assert_eq!(letters_from_col(52), "BA");
        assert_eq!(letters_from_col(701), "ZZ");
        assert_eq!(letters_from_col(702), "AAA");
    }
}
