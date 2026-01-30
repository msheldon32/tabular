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
    Unknown(String),
}

impl Command {
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();

        match trimmed {
            "w" => Command::Write,
            "q" => Command::Quit,
            "q!" => Command::ForceQuit,
            "wq" => Command::WriteQuit,
            "addcol" => Command::AddColumn,
            "delcol" => Command::DeleteColumn,
            "header" => Command::ToggleHeader,
            "calc" => Command::Calc,
            _ => Command::Unknown(trimmed.to_string()),
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
