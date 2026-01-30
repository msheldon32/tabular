
use regex::Regex;
use crate::util::{CellRef, parse_cell_ref};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Write,
    Quit,
    ForceQuit,
    WriteQuit,
    AddColumn,
    DeleteColumn,
    ToggleHeader,
    Calc,
    NavigateRow(usize),
    NavigateCell(CellRef),
    Unknown(String),
}

impl Command {
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();

        if let Ok(row_dest) = input.parse::<usize>() {
            return Some(Command::NavigateRow(row_dest-1));
        }

        let cell_re = Regex::new(r"[A-Z]+\d+").unwrap();

        if cell_re.is_match(input) {
            return Some(Command::NavigateCell(parse_cell_ref(input)?));
        }

        match trimmed {
            "w" => Some(Command::Write),
            "q" => Some(Command::Quit),
            "q!" => Some(Command::ForceQuit),
            "wq" => Some(Command::WriteQuit),
            "addcol" => Some(Command::AddColumn),
            "delcol" => Some(Command::DeleteColumn),
            "header" => Some(Command::ToggleHeader),
            "calc" => Some(Command::Calc),
            _ => Some(Command::Unknown(trimmed.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commands() {
        assert_eq!(Command::parse("w"), Command::Write);
        assert_eq!(Command::parse("q"), Command::Quit);
        assert_eq!(Command::parse("q!"), Command::ForceQuit);
        assert_eq!(Command::parse("wq"), Command::WriteQuit);
        assert_eq!(Command::parse("addcol"), Command::AddColumn);
        assert_eq!(Command::parse("delcol"), Command::DeleteColumn);
        assert_eq!(Command::parse("header"), Command::ToggleHeader);
        assert_eq!(Command::parse("calc"), Command::Calc);
        assert_eq!(
            Command::parse("unknown"),
            Command::Unknown("unknown".to_string())
        );
    }

    #[test]
    fn test_parse_with_whitespace() {
        assert_eq!(Command::parse("  w  "), Command::Write);
        assert_eq!(Command::parse("\tq\n"), Command::Quit);
    }
}
