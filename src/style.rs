use ratatui::style::{Color, Modifier, Style as RatStyle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Color that can be serialized/deserialized
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeColor {
    /// Named color: "red", "blue", "cyan", etc.
    Named(NamedColor),
    /// RGB color: [255, 128, 0]
    Rgb([u8; 3]),
    /// 256-color index: 42
    Indexed(u8),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
    Reset,
}

impl From<ThemeColor> for Color {
    fn from(tc: ThemeColor) -> Color {
        match tc {
            ThemeColor::Named(n) => match n {
                NamedColor::Black => Color::Black,
                NamedColor::Red => Color::Red,
                NamedColor::Green => Color::Green,
                NamedColor::Yellow => Color::Yellow,
                NamedColor::Blue => Color::Blue,
                NamedColor::Magenta => Color::Magenta,
                NamedColor::Cyan => Color::Cyan,
                NamedColor::Gray => Color::Gray,
                NamedColor::DarkGray => Color::DarkGray,
                NamedColor::LightRed => Color::LightRed,
                NamedColor::LightGreen => Color::LightGreen,
                NamedColor::LightYellow => Color::LightYellow,
                NamedColor::LightBlue => Color::LightBlue,
                NamedColor::LightMagenta => Color::LightMagenta,
                NamedColor::LightCyan => Color::LightCyan,
                NamedColor::White => Color::White,
                NamedColor::Reset => Color::Reset,
            },
            ThemeColor::Rgb([r, g, b]) => Color::Rgb(r, g, b),
            ThemeColor::Indexed(i) => Color::Indexed(i),
        }
    }
}

/// Style definition for a single element
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElementStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<ThemeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<ThemeColor>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub dim: bool,
}

impl Default for ElementStyle {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            dim: false,
        }
    }
}

impl ElementStyle {
    pub fn fg(color: ThemeColor) -> Self {
        Self { fg: Some(color), ..Default::default() }
    }

    pub fn bg(color: ThemeColor) -> Self {
        Self { bg: Some(color), ..Default::default() }
    }

    pub fn with_fg(mut self, color: ThemeColor) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn with_bg(mut self, color: ThemeColor) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn with_dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub fn to_ratatui(&self) -> RatStyle {
        let mut style = RatStyle::default();
        if let Some(fg) = self.fg {
            style = style.fg(fg.into());
        }
        if let Some(bg) = self.bg {
            style = style.bg(bg.into());
        }
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if self.underline {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if self.dim {
            style = style.add_modifier(Modifier::DIM);
        }
        style
    }
}

/// Complete theme configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,

    // Background color for the entire UI
    #[serde(default)]
    pub background: Option<ThemeColor>,

    // Table cells
    pub cell: ElementStyle,
    pub cell_cursor: ElementStyle,
    pub cell_selection: ElementStyle,
    pub cell_match: ElementStyle,

    // Row/column headers
    pub header_col: ElementStyle,
    pub header_row: ElementStyle,
    pub row_number: ElementStyle,
    pub row_number_cursor: ElementStyle,

    // Status bar
    pub status_bar: ElementStyle,
    pub status_mode_normal: ElementStyle,
    pub status_mode_insert: ElementStyle,
    pub status_mode_visual: ElementStyle,
    pub status_mode_command: ElementStyle,

    // Messages
    pub message_info: ElementStyle,
    pub message_warning: ElementStyle,
    pub message_error: ElementStyle,

    // Filter status
    pub filter_status: ElementStyle,

    // Command line
    pub command_line: ElementStyle,
    pub command_prompt: ElementStyle,

    // Grid
    #[serde(default)]
    pub show_grid: bool,
    pub grid: ElementStyle,
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

impl Theme {
    /// Dark theme with black background
    pub fn dark() -> Self {
        use NamedColor::*;
        Self {
            name: "dark".to_string(),
            background: Some(ThemeColor::Named(Black)),
            cell: ElementStyle::fg(ThemeColor::Named(White)),
            cell_cursor: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(LightCyan))
                .with_bold(),
            cell_selection: ElementStyle::fg(ThemeColor::Named(White))
                .with_bg(ThemeColor::Named(DarkGray)),
            cell_match: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(Yellow)),
            header_col: ElementStyle::fg(ThemeColor::Named(LightGreen)).with_bold(),
            header_row: ElementStyle::fg(ThemeColor::Named(LightCyan)).with_bold(),
            row_number: ElementStyle::fg(ThemeColor::Named(Gray)),
            row_number_cursor: ElementStyle::fg(ThemeColor::Named(LightYellow)).with_bold(),
            status_bar: ElementStyle::fg(ThemeColor::Named(White))
                .with_bg(ThemeColor::Named(DarkGray)),
            status_mode_normal: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(LightBlue))
                .with_bold(),
            status_mode_insert: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(LightGreen))
                .with_bold(),
            status_mode_visual: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(LightMagenta))
                .with_bold(),
            status_mode_command: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(LightYellow))
                .with_bold(),
            message_info: ElementStyle::fg(ThemeColor::Named(White)),
            message_warning: ElementStyle::fg(ThemeColor::Named(LightYellow)),
            message_error: ElementStyle::fg(ThemeColor::Named(LightRed)).with_bold(),
            filter_status: ElementStyle::fg(ThemeColor::Named(Blue)).with_bold(),
            command_line: ElementStyle::fg(ThemeColor::Named(White)),
            command_prompt: ElementStyle::fg(ThemeColor::Named(LightCyan)),
            show_grid: false,
            grid: ElementStyle::fg(ThemeColor::Named(DarkGray)),
        }
    }

    /// Light theme (default)
    pub fn light() -> Self {
        use NamedColor::*;
        Self {
            name: "light".to_string(),
            background: None, // Use terminal default
            cell: ElementStyle::fg(ThemeColor::Named(Black)),
            cell_cursor: ElementStyle::fg(ThemeColor::Named(White))
                .with_bg(ThemeColor::Named(Blue))
                .with_bold(),
            cell_selection: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(LightCyan)),
            cell_match: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(Yellow)),
            header_col: ElementStyle::fg(ThemeColor::Named(Blue)).with_bold(),
            header_row: ElementStyle::fg(ThemeColor::Named(Green)).with_bold(),
            row_number: ElementStyle::fg(ThemeColor::Named(Gray)),
            row_number_cursor: ElementStyle::fg(ThemeColor::Named(Blue)).with_bold(),
            status_bar: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(Gray)),
            status_mode_normal: ElementStyle::fg(ThemeColor::Named(White))
                .with_bg(ThemeColor::Named(Blue))
                .with_bold(),
            status_mode_insert: ElementStyle::fg(ThemeColor::Named(White))
                .with_bg(ThemeColor::Named(Green))
                .with_bold(),
            status_mode_visual: ElementStyle::fg(ThemeColor::Named(White))
                .with_bg(ThemeColor::Named(Magenta))
                .with_bold(),
            status_mode_command: ElementStyle::fg(ThemeColor::Named(Black))
                .with_bg(ThemeColor::Named(Yellow))
                .with_bold(),
            message_info: ElementStyle::fg(ThemeColor::Named(Black)),
            message_warning: ElementStyle::fg(ThemeColor::Named(Yellow)),
            message_error: ElementStyle::fg(ThemeColor::Named(Red)).with_bold(),
            filter_status: ElementStyle::fg(ThemeColor::Named(Blue)).with_bold(),
            command_line: ElementStyle::fg(ThemeColor::Named(Black)),
            command_prompt: ElementStyle::fg(ThemeColor::Named(Blue)),
            show_grid: true,
            grid: ElementStyle::fg(ThemeColor::Named(Gray)),
        }
    }

    /// Solarized dark theme
    pub fn solarized_dark() -> Self {
        // Solarized colors
        let base03 = ThemeColor::Rgb([0, 43, 54]);
        let base02 = ThemeColor::Rgb([7, 54, 66]);
        let base01 = ThemeColor::Rgb([88, 110, 117]);
        let base0 = ThemeColor::Rgb([131, 148, 150]);
        let base1 = ThemeColor::Rgb([147, 161, 161]);
        let yellow = ThemeColor::Rgb([181, 137, 0]);
        let orange = ThemeColor::Rgb([203, 75, 22]);
        let red = ThemeColor::Rgb([220, 50, 47]);
        let magenta = ThemeColor::Rgb([211, 54, 130]);
        let blue = ThemeColor::Rgb([38, 139, 210]);
        let cyan = ThemeColor::Rgb([42, 161, 152]);
        let green = ThemeColor::Rgb([133, 153, 0]);

        Self {
            name: "solarized-dark".to_string(),
            background: Some(base03),
            cell: ElementStyle::fg(base0),
            cell_cursor: ElementStyle::fg(base03).with_bg(blue).with_bold(),
            cell_selection: ElementStyle::fg(base0).with_bg(base02),
            cell_match: ElementStyle::fg(base03).with_bg(yellow),
            header_col: ElementStyle::fg(cyan).with_bold(),
            header_row: ElementStyle::fg(green).with_bold(),
            row_number: ElementStyle::fg(base01),
            row_number_cursor: ElementStyle::fg(yellow).with_bold(),
            status_bar: ElementStyle::fg(base1).with_bg(base02),
            status_mode_normal: ElementStyle::fg(base03).with_bg(blue).with_bold(),
            status_mode_insert: ElementStyle::fg(base03).with_bg(green).with_bold(),
            status_mode_visual: ElementStyle::fg(base03).with_bg(magenta).with_bold(),
            status_mode_command: ElementStyle::fg(base03).with_bg(yellow).with_bold(),
            message_info: ElementStyle::fg(base0),
            message_warning: ElementStyle::fg(orange),
            message_error: ElementStyle::fg(red).with_bold(),
            filter_status: ElementStyle::fg(blue).with_bold(),
            command_line: ElementStyle::fg(base0),
            command_prompt: ElementStyle::fg(cyan),
            show_grid: false,
            grid: ElementStyle::fg(base01),
        }
    }

    /// Load theme from TOML file
    pub fn from_file(path: &PathBuf) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read theme file: {}", e))?;
        toml::from_str(&content)
            .map_err(|e| format!("Failed to parse theme file: {}", e))
    }

    /// Get theme by name
    pub fn by_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "dark" => Some(Self::dark()),
            "light" => Some(Self::light()),
            "solarized" | "solarized-dark" => Some(Self::solarized_dark()),
            _ => None,
        }
    }

    /// List available built-in themes
    pub fn builtin_names() -> &'static [&'static str] {
        &["dark", "light", "solarized-dark"]
    }
}

/// Runtime style manager
pub struct Style {
    pub theme: Theme,
}

impl Style {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn with_theme(theme: Theme) -> Self {
        Self { theme }
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn toggle_grid(&mut self) {
        self.theme.show_grid = !self.theme.show_grid;
    }

    pub fn has_grid(&self) -> bool {
        self.theme.show_grid
    }

    // Convenience accessors that return ratatui styles
    pub fn cell(&self) -> RatStyle {
        self.theme.cell.to_ratatui()
    }

    pub fn cell_cursor(&self) -> RatStyle {
        self.theme.cell_cursor.to_ratatui()
    }

    pub fn cell_selection(&self) -> RatStyle {
        self.theme.cell_selection.to_ratatui()
    }

    pub fn cell_match(&self) -> RatStyle {
        self.theme.cell_match.to_ratatui()
    }

    pub fn header_col(&self) -> RatStyle {
        self.theme.header_col.to_ratatui()
    }

    pub fn header_row(&self) -> RatStyle {
        self.theme.header_row.to_ratatui()
    }

    pub fn row_number(&self) -> RatStyle {
        self.theme.row_number.to_ratatui()
    }

    pub fn row_number_cursor(&self) -> RatStyle {
        self.theme.row_number_cursor.to_ratatui()
    }

    pub fn status_bar(&self) -> RatStyle {
        self.theme.status_bar.to_ratatui()
    }

    pub fn status_mode(&self, mode: &crate::mode::Mode) -> RatStyle {
        use crate::mode::Mode;
        match mode {
            Mode::Normal => self.theme.status_mode_normal.to_ratatui(),
            Mode::Insert => self.theme.status_mode_insert.to_ratatui(),
            Mode::Visual | Mode::VisualRow | Mode::VisualCol => {
                self.theme.status_mode_visual.to_ratatui()
            }
            Mode::Command | Mode::Search => self.theme.status_mode_command.to_ratatui(),
        }
    }

    pub fn message_info(&self) -> RatStyle {
        self.theme.message_info.to_ratatui()
    }

    pub fn message_warning(&self) -> RatStyle {
        self.theme.message_warning.to_ratatui()
    }

    pub fn message_error(&self) -> RatStyle {
        self.theme.message_error.to_ratatui()
    }

    pub fn filter_status(&self) -> RatStyle {
        self.theme.filter_status.to_ratatui()
    }

    pub fn command_line(&self) -> RatStyle {
        self.theme.command_line.to_ratatui()
    }

    pub fn command_prompt(&self) -> RatStyle {
        self.theme.command_prompt.to_ratatui()
    }

    pub fn grid(&self) -> RatStyle {
        self.theme.grid.to_ratatui()
    }

    pub fn background(&self) -> Option<Color> {
        self.theme.background.map(|c| c.into())
    }
}
