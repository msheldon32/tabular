pub mod command;
pub mod insert;
pub mod visual;
pub mod search;
pub mod normal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Visual,
    VisualRow,
    VisualCol,
    Search,
}

impl Mode {
    pub fn display_name(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
            Mode::Visual => "VISUAL",
            Mode::VisualRow => "VISUAL (ROW)",
            Mode::VisualCol => "VISUAL (COL)",
            Mode::Search => "SEARCH",
        }
    }

    pub fn is_visual(&self) -> bool {
        matches!(self, Mode::Visual | Mode::VisualRow | Mode::VisualCol)
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Normal
    }
}

#[cfg(test)]
mod test;
