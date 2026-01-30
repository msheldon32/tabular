use ratatui::style as rats;

pub struct Style {
    pub has_grid: bool,
    pub header_style: rats::Style,
    pub highlight_number_cell_style: rats::Style,
    pub number_cell_style: rats::Style,
    pub cursor_cell_style: rats::Style, 
    pub match_cell_style: rats::Style,
    pub header_row_style: rats::Style,
    pub default_cell_style: rats::Style
}

impl Style {
    pub fn new() -> Self {
        Self {
            has_grid: false,
            header_style: rats::Style::default().fg(rats::Color::Cyan).add_modifier(rats::Modifier::BOLD),
            highlight_number_cell_style: rats::Style::default().fg(rats::Color::Yellow).add_modifier(rats::Modifier::BOLD),
            number_cell_style: rats::Style::default().fg(rats::Color::DarkGray),
            cursor_cell_style: rats::Style::default().bg(rats::Color::Blue).fg(rats::Color::White).add_modifier(rats::Modifier::BOLD),
            match_cell_style: rats::Style::default().bg(rats::Color::Yellow).fg(rats::Color::Black),
            header_row_style: rats::Style::default().fg(rats::Color::Green).add_modifier(rats::Modifier::BOLD),
            default_cell_style: rats::Style::default()
        }
    }

    pub fn toggle_grid(&mut self) {
        self.has_grid = !self.has_grid;
    }

    pub fn bottom_margin(&mut self) -> u16 {
        return 0;
    }
}
