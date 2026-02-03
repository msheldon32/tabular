use regex::Regex;


use crate::util::{CellRef, parse_cell_ref};
use crate::rowmanager::FilterType;

use crate::predicate::parse_predicate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceScope {
    All,           // %s - entire table
    Selection,     // s - visual selection only
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceCommand {
    pub pattern: String,
    pub replacement: String,
    pub global: bool,      // /g flag - replace all occurrences in each cell
    pub scope: ReplaceScope,
}

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
    Sort,           // Sort rows by current column, ascending
    SortDesc,       // Sort rows by current column, descending
    SortRow,        // Sort columns by current row, ascending
    SortRowDesc,    // Sort columns by current row, descending
    Grid,
    Theme(String),  // Set theme by name
    ThemeList,      // List available themes
    Replace(ReplaceCommand),
    NavigateRow(usize),
    NavigateCell(CellRef),
    Fork,
    Clip,           // Copy yank to system clipboard
    SysPaste,       // Yank from system clipboard
    PluginList,     // List loaded plugins
    Precision(Option<usize>),  // Set display precision for numbers (None = auto)
    Custom { name: String, args: Vec<String> },
    Filter(FilterType),
    Unknown(String),
}

impl Command {
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();

        // Check for substitute/replace command: %s/old/new/g or s/old/new/g
        if let Some(replace_cmd) = Self::parse_replace(trimmed) {
            return Some(Command::Replace(replace_cmd));
        }

        if let Ok(row_dest) = input.parse::<usize>() {
            return Some(Command::NavigateRow(row_dest-1));
        }

        let cell_re = Regex::new(r"[A-Z]+\d+").unwrap();

        if cell_re.is_match(input) {
            return Some(Command::NavigateCell(parse_cell_ref(input)?));
        }

        // Check for theme command with argument
        if let Some(theme_name) = trimmed.strip_prefix("theme ") {
            return Some(Command::Theme(theme_name.trim().to_string()));
        }

        if let Some(filter_args) = trimmed.strip_prefix("filter ") {
            let predicate = parse_predicate(filter_args.to_string());
            match predicate {
                Some(pred) => {
                    return Some(Command::Filter(FilterType::PredicateFilter(pred)));
                }
                None => {
                    return None;
                }
            }
        }

        // Check for precision command: prec N or precision N (or just prec/precision for auto)
        if let Some(rest) = trimmed.strip_prefix("prec ").or_else(|| trimmed.strip_prefix("precision ")) {
            let rest = rest.trim();
            if rest.is_empty() || rest == "auto" {
                return Some(Command::Precision(None));
            }
            if let Ok(n) = rest.parse::<usize>() {
                return Some(Command::Precision(Some(n)));
            }
            // Invalid precision value, fall through to unknown
        }
        if trimmed == "prec" || trimmed == "precision" {
            return Some(Command::Precision(None));
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
            "sort" => Some(Command::Sort),
            "fork" => Some(Command::Fork),
            "sortd" | "sort!" => Some(Command::SortDesc),
            "sortr" => Some(Command::SortRow),
            "sortrd" | "sortr!" => Some(Command::SortRowDesc),
            "grid" => Some(Command::Grid),
            "theme" | "themes" => Some(Command::ThemeList),
            "clip" | "cp" => Some(Command::Clip),
            "sp" | "syspaste" => Some(Command::SysPaste),
            "plugins" => Some(Command::PluginList),
            "fibfilter" => Some(Command::Filter(FilterType::Fibonacci)),
            "nofilter" => Some(Command::Filter(FilterType::Default)),
            _ => Some(Command::Unknown(trimmed.to_string())),
        }
    }

    /// Parse a substitute/replace command
    /// Formats: %s/old/new/g, s/old/new/g, %s/old/new, s/old/new
    fn parse_replace(input: &str) -> Option<ReplaceCommand> {
        // Match %s/.../.../[g] or s/.../.../[g]
        // Use a regex that handles the delimiter
        let re = Regex::new(r"^(%)?s/([^/]*)/([^/]*)(/g)?$").unwrap();

        if let Some(caps) = re.captures(input) {
            let scope = if caps.get(1).is_some() {
                ReplaceScope::All
            } else {
                ReplaceScope::Selection
            };
            let pattern = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
            let replacement = caps.get(3).map(|m| m.as_str()).unwrap_or("").to_string();
            let global = caps.get(4).is_some();

            if pattern.is_empty() {
                return None;
            }

            return Some(ReplaceCommand {
                pattern,
                replacement,
                global,
                scope,
            });
        }

        None
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
    fn test_parse_sort_commands() {
        assert_eq!(Command::parse("sort"), Some(Command::Sort));
        assert_eq!(Command::parse("sortd"), Some(Command::SortDesc));
        assert_eq!(Command::parse("sort!"), Some(Command::SortDesc));
        assert_eq!(Command::parse("sortr"), Some(Command::SortRow));
        assert_eq!(Command::parse("sortrd"), Some(Command::SortRowDesc));
        assert_eq!(Command::parse("sortr!"), Some(Command::SortRowDesc));
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
