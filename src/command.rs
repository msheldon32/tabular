
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
    fn test_parse_basic_commands() {
        assert_eq!(Command::parse("w"), Some(Command::Write));
        assert_eq!(Command::parse("q"), Some(Command::Quit));
        assert_eq!(Command::parse("q!"), Some(Command::ForceQuit));
        assert_eq!(Command::parse("wq"), Some(Command::WriteQuit));
        assert_eq!(Command::parse("addcol"), Some(Command::AddColumn));
        assert_eq!(Command::parse("delcol"), Some(Command::DeleteColumn));
        assert_eq!(Command::parse("header"), Some(Command::ToggleHeader));
        assert_eq!(Command::parse("calc"), Some(Command::Calc));
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(
            Command::parse("unknown"),
            Some(Command::Unknown("unknown".to_string()))
        );
        assert_eq!(
            Command::parse("foobar"),
            Some(Command::Unknown("foobar".to_string()))
        );
    }

    #[test]
    fn test_parse_with_whitespace() {
        assert_eq!(Command::parse("  w  "), Some(Command::Write));
        assert_eq!(Command::parse("\tq\n"), Some(Command::Quit));
        assert_eq!(Command::parse("  q!  "), Some(Command::ForceQuit));
    }

    #[test]
    fn test_parse_row_navigation() {
        assert_eq!(Command::parse("1"), Some(Command::NavigateRow(0)));
        assert_eq!(Command::parse("10"), Some(Command::NavigateRow(9)));
        assert_eq!(Command::parse("100"), Some(Command::NavigateRow(99)));
    }

    #[test]
    fn test_parse_cell_navigation() {
        let cmd = Command::parse("A1");
        assert!(matches!(cmd, Some(Command::NavigateCell(_))));

        if let Some(Command::NavigateCell(cell)) = cmd {
            assert_eq!(cell.row, 0);
            assert_eq!(cell.col, 0);
        }

        let cmd = Command::parse("B5");
        if let Some(Command::NavigateCell(cell)) = cmd {
            assert_eq!(cell.row, 4);
            assert_eq!(cell.col, 1);
        }
    }

    #[test]
    fn test_parse_empty() {
        // Empty string should return Unknown with empty string
        assert_eq!(Command::parse(""), Some(Command::Unknown(String::new())));
    }
}
