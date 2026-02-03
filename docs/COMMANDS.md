# Commands

Enter command mode by pressing `:` in normal mode.

## File Operations

| Command | Action |
|---------|--------|
| `:w` | Save file |
| `:q` | Quit (fails if unsaved changes) |
| `:q!` | Force quit without saving |
| `:wq` | Save and quit |

## Table Structure

| Command | Action |
|---------|--------|
| `:addcol` | Add column after current |
| `:delcol` | Delete current column |
| `:header` | Toggle header mode |

## Sorting

| Command | Action |
|---------|--------|
| `:sort` | Sort rows by current column (ascending) |
| `:sortd` | Sort rows by current column (descending) |
| `:sortr` | Sort columns by current row (ascending) |
| `:sortrd` | Sort columns by current row (descending) |

**Automatic type detection**: Tabular probes the column to determine if it contains numeric or text data:
- **Numeric sort**: If the majority of non-empty cells are numbers, sorting is done numerically
- **Text sort**: Otherwise, sorting is case-insensitive alphabetical

**Formatted number recognition**: Currency and percentages are recognized as numbers:
- Currency: `$1,234.56`, `€500`, `-$100`, `($50)`
- Percentages: `15%`, `3.5%`
- Numbers with commas: `1,234,567`

**Header preservation**: When header mode is enabled (default), the first row is kept in place.

## Filtering

| Command | Action |
|---------|--------|
| `:filter <op> <val>` | Filter rows by current column |
| `:nofilter` | Remove filter and show all rows |

### Operators

| Operator | Description |
|----------|-------------|
| `=` | Equal to |
| `!` | Not equal to |
| `<` | Less than |
| `<=` | Less than or equal to |
| `>` | Greater than |
| `>=` | Greater than or equal to |

### Examples

```
:filter > 100       # Show rows where current column > 100
:filter = active    # Show rows where current column equals "active"
:filter ! pending   # Show rows where current column is not "pending"
:filter >= 50       # Show rows where current column >= 50
```

## Find and Replace

| Command | Description |
|---------|-------------|
| `:s/old/new/` | Replace first occurrence in current cell |
| `:s/old/new/g` | Replace all occurrences in current cell |
| `:%s/old/new/` | Replace first occurrence in each cell |
| `:%s/old/new/g` | Replace all occurrences in all cells |

With visual selection, `:s/old/new/` operates on selected cells only.

**Note:** Replacements are literal string matches, not regular expressions.

## Display

| Command | Action |
|---------|--------|
| `:grid` | Toggle grid lines |
| `:prec [N]` | Set display precision (N decimal places, or `auto`) |
| `:theme [name]` | Set color theme (dark, light, solarized-dark) |
| `:themes` | List available themes |

## Clipboard

| Command | Action |
|---------|--------|
| `:clip` | Copy yanked data to system clipboard |
| `:sp` | Yank from system clipboard (then `p` to paste) |

## Navigation

| Command | Action |
|---------|--------|
| `:[NUMBER]` | Jump to row NUMBER |
| `:[CELL]` | Jump to CELL (e.g., `:A1`, `:B5`) |

## Formulas

| Command | Action |
|---------|--------|
| `:calc` | Evaluate all formulas |

## Plugins

| Command | Action |
|---------|--------|
| `:plugins` | List all loaded plugins |
| `:<plugin_name>` | Run a plugin command |
