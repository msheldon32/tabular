use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Color representation for canvas
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum CanvasColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
}

impl CanvasColor {
    pub fn to_ratatui(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            CanvasColor::Black => Color::Black,
            CanvasColor::Red => Color::Red,
            CanvasColor::Green => Color::Green,
            CanvasColor::Yellow => Color::Yellow,
            CanvasColor::Blue => Color::Blue,
            CanvasColor::Magenta => Color::Magenta,
            CanvasColor::Cyan => Color::Cyan,
            CanvasColor::White => Color::White,
            CanvasColor::Gray => Color::Gray,
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "black" => Some(CanvasColor::Black),
            "red" => Some(CanvasColor::Red),
            "green" => Some(CanvasColor::Green),
            "yellow" => Some(CanvasColor::Yellow),
            "blue" => Some(CanvasColor::Blue),
            "magenta" => Some(CanvasColor::Magenta),
            "cyan" => Some(CanvasColor::Cyan),
            "white" => Some(CanvasColor::White),
            "gray" | "grey" => Some(CanvasColor::Gray),
            _ => None,
        }
    }
}


/// A single item that can be displayed on the canvas
#[derive(Debug, Clone)]
pub enum CanvasItem {
    /// Plain text line
    Text(String),
    /// Styled text with color
    StyledText {
        text: String,
        fg: Option<Color>,
        bg: Option<Color>,
        bold: bool,
    },
    /// Horizontal separator
    Separator,
    /// Header/title text (bold, centered)
    Header(String),
    /// Image represented as ASCII art or block characters
    /// Each string is one row of the image
    Image {
        rows: Vec<String>,
        title: Option<String>,
    },
    /// Empty line
    Blank,
}

/// Canvas overlay for displaying rich content
#[derive(Debug, Clone)]
pub struct Canvas {
    /// Items to display
    items: Vec<CanvasItem>,
    /// Title for the canvas window
    title: String,
    /// Whether the canvas is visible
    pub visible: bool,
    /// Scroll offset for long content
    scroll: usize,
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            title: "Canvas".to_string(),
            visible: false,
            scroll: 0,
        }
    }

    /// Show the canvas
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the canvas
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Clear all content
    pub fn clear(&mut self) {
        self.items.clear();
        self.scroll = 0;
    }

    /// Set the title
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Add a plain text line
    pub fn add_text(&mut self, text: impl Into<String>) {
        self.items.push(CanvasItem::Text(text.into()));
    }

    /// Add styled text
    pub fn add_styled_text(
        &mut self,
        text: impl Into<String>,
        fg: Option<Color>,
        bg: Option<Color>,
        bold: bool,
    ) {
        self.items.push(CanvasItem::StyledText {
            text: text.into(),
            fg,
            bg,
            bold,
        });
    }

    /// Add a header line
    pub fn add_header(&mut self, text: impl Into<String>) {
        self.items.push(CanvasItem::Header(text.into()));
    }

    /// Add a separator line
    pub fn add_separator(&mut self) {
        self.items.push(CanvasItem::Separator);
    }

    /// Add a blank line
    pub fn add_blank(&mut self) {
        self.items.push(CanvasItem::Blank);
    }

    /// Add an ASCII art image
    pub fn add_image(&mut self, rows: Vec<String>, title: Option<String>) {
        self.items.push(CanvasItem::Image { rows, title });
    }

    /// Add a simple box/rectangle drawing
    pub fn add_box(&mut self, width: usize, height: usize, fill_char: char) {
        let mut rows = Vec::with_capacity(height);
        let top_bottom = format!("┌{}┐", "─".repeat(width));
        let middle = format!("│{}│", fill_char.to_string().repeat(width));
        let bottom = format!("└{}┘", "─".repeat(width));

        rows.push(top_bottom);
        for _ in 0..height.saturating_sub(2) {
            rows.push(middle.clone());
        }
        if height > 1 {
            rows.push(bottom);
        }

        self.items.push(CanvasItem::Image { rows, title: None });
    }

    /// Scroll up
    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    /// Scroll down
    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_add(amount);
    }

    /// Render the canvas as an overlay
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // Calculate overlay size (80% of screen, centered)
        let overlay_width = (area.width * 80 / 100).max(40).min(area.width - 4);
        let overlay_height = (area.height * 80 / 100).max(10).min(area.height - 4);

        let overlay_x = (area.width - overlay_width) / 2;
        let overlay_y = (area.height - overlay_height) / 2;

        let overlay_area = Rect::new(overlay_x, overlay_y, overlay_width, overlay_height);

        // Clear the area behind the overlay
        frame.render_widget(Clear, overlay_area);

        // Build lines from items
        let mut lines: Vec<Line> = Vec::new();

        for item in &self.items {
            match item {
                CanvasItem::Text(text) => {
                    lines.push(Line::from(text.as_str()));
                }
                CanvasItem::StyledText { text, fg, bg, bold } => {
                    let mut style = Style::default();
                    if let Some(fg_color) = fg {
                        style = style.fg(*fg_color);
                    }
                    if let Some(bg_color) = bg {
                        style = style.bg(*bg_color);
                    }
                    if *bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    lines.push(Line::from(Span::styled(text.as_str(), style)));
                }
                CanvasItem::Header(text) => {
                    let style = Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD);
                    lines.push(Line::from(Span::styled(text.as_str(), style)));
                }
                CanvasItem::Separator => {
                    let sep = "─".repeat(overlay_width.saturating_sub(4) as usize);
                    lines.push(Line::from(Span::styled(
                        sep,
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                CanvasItem::Blank => {
                    lines.push(Line::from(""));
                }
                CanvasItem::Image { rows, title } => {
                    if let Some(t) = title {
                        lines.push(Line::from(Span::styled(
                            t.as_str(),
                            Style::default().fg(Color::Cyan),
                        )));
                    }
                    for row in rows {
                        lines.push(Line::from(row.as_str()));
                    }
                }
            }
        }

        // Apply scroll offset
        let visible_height = overlay_height.saturating_sub(2) as usize; // Account for border
        let max_scroll = lines.len().saturating_sub(visible_height);
        let scroll = self.scroll.min(max_scroll);
        let visible_lines: Vec<Line> = lines.into_iter().skip(scroll).collect();

        // Create the help text for the title
        let title_with_help = format!(" {} [q/Esc to close, j/k to scroll] ", self.title);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title_with_help)
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(visible_lines)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, overlay_area);
    }
}
